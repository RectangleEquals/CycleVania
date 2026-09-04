//! **State graphs, and the un-softlockable check over them.**
//!
//! A bounded state machine: nodes are settings of one variable, edges are transitions with what they
//! cost. ⚠ **One graph, two problems** — it is the authoring surface for a multi-element puzzle *and*
//! for the world-state axis of the solve.
//!
//! # Why this is core rather than the editor's
//!
//! ⚠ **A host has every reason to ask it.** *"Can this world strand a player?"* is the same question
//! [`crate::softlock`] answers over the mission graph; this answers it over the other graph. The editor
//! **draws the result** and owns none of it — a check is not a view.
//!
//! ▶ **It was in the editor for three milestones**, because the editor was the first caller. Being
//! the first caller does not make you the owner.
//!
//! # Re-enterability, not reachability
//!
//! ⚠ **The question is not *"can I get to `open`"* but *"having got there, can I get back"*.** A
//! state machine where every state is reachable from the initial one can still strand a player the
//! moment they enter the wrong one, and that is exactly the softlock this finds.
//!
//! ⚠ **Unlocks are monotone and states are not.** That is the whole reason this analysis exists
//! separately: [`crate::softlock`] may assume progress never reverses, and here *"being too slow undoes
//! progress"* is an ordinary edge.
//!
//! # The hard case is a cycle that does not close
//!
//! ⚠ **A control that is itself state-gated** is the case nobody catches by reading — the transition
//! back exists, and it needs something only the far state grants.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// One setting of the variable.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct State {
    /// Its name, which is also its identity in a transition.
    pub name: String,
    /// Is this where the variable starts?
    pub initial: bool,
    /// The developer's words.
    pub doc: String,
}

/// A directed move between two settings.
///
/// ⚠ **Directed, and the reverse is a separate transition.** *"Being too slow undoes progress"* is a
/// move **back**, which a monotone unlock model cannot express — and a `bidirectional` flag would make
/// the asymmetric case the awkward one to write, when it is the case the format exists for.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Transition {
    /// Where it starts.
    pub from: String,
    /// Where it lands.
    pub to: String,
    /// What must be held to take it. Empty means anyone may.
    pub requires: Vec<String>,
    /// What performs it, for the label.
    pub via: String,
}

/// What the check found.
///
/// ⚠ **A finding names states, not indices.** The developer is looking at a picture with names on it,
/// and a message about `state[2]` would make them count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finding {
    /// A state nothing can reach from the initial one.
    ///
    /// ⚠ **Distinct from a dead end.** *"Nobody can get here"* is usually a mistake; *"nobody can
    /// leave"* is usually a softlock, and telling a developer the wrong one sends them to the wrong end
    /// of the graph.
    Inaccessible { state: String },
    /// ⚠ **A state that cannot be left at all** — the plain softlock.
    DeadEnd { state: String },
    /// A state that can only be left by holding something.
    ///
    /// ⚠ **A warning, not an error.** Not an error, because gating a way back is a legitimate
    /// design — it is a *potential* softlock, and how many paths it affects is what tells a developer
    /// whether to care.
    ExitGated {
        state: String,
        requires: Vec<String>,
        paths_affected: usize,
    },
    /// No state is marked initial, or more than one is.
    ///
    /// ⚠ **Both are one finding**, because both make *"where does this variable start"* unanswerable.
    InitialUnclear { count: usize },
    /// A transition naming a state the graph does not have.
    UnknownState { transition: String, missing: String },
}

impl Finding {
    /// ⚠ **Does this stop a build, or only warn on the drawing?**
    ///
    /// A gated exit is a design decision; an unreachable state or a dead end is a mistake. Blocking on
    /// the first would make the check something developers turn off.
    pub fn blocks(&self) -> bool {
        !matches!(self, Finding::ExitGated { .. })
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Finding::Inaccessible { state } => {
                write!(f, "`{state}` cannot be reached from the initial state")
            }
            Finding::DeadEnd { state } => write!(
                f,
                "`{state}` has no transition out — entering it strands the variable"
            ),
            Finding::ExitGated {
                state,
                requires,
                paths_affected,
            } => write!(
                f,
                "`{state}` is accessible but leaving it needs [{}] — potential softlock, \
                 {paths_affected} path(s) affected",
                requires.join(", ")
            ),
            Finding::InitialUnclear { count } => write!(
                f,
                "{count} states are marked initial — exactly one must be, or where the variable starts \
                 has no answer"
            ),
            Finding::UnknownState {
                transition,
                missing,
            } => write!(f, "transition {transition} names `{missing}`, which is not a state"),
        }
    }
}

/// A `.cvstate` graph.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateGraph {
    /// What the graph settles.
    pub variable: String,
    /// The settings.
    pub states: Vec<State>,
    /// The moves.
    pub transitions: Vec<Transition>,
}

impl StateGraph {
    /// An empty graph over a variable.
    pub fn new(variable: impl Into<String>) -> Self {
        StateGraph {
            variable: variable.into(),
            ..StateGraph::default()
        }
    }

    /// Add a state.
    pub fn state(mut self, name: &str, initial: bool) -> Self {
        self.states.push(State {
            name: name.into(),
            initial,
            doc: String::new(),
        });
        self
    }

    /// Add a transition.
    pub fn transition(mut self, from: &str, to: &str, requires: &[&str], via: &str) -> Self {
        self.transitions.push(Transition {
            from: from.into(),
            to: to.into(),
            requires: requires.iter().map(|s| (*s).to_string()).collect(),
            via: via.into(),
        });
        self
    }

    /// The initial state's name, when exactly one is marked.
    pub fn initial(&self) -> Option<&str> {
        let marked: Vec<&State> = self.states.iter().filter(|s| s.initial).collect();
        match marked.as_slice() {
            [one] => Some(&one.name),
            _ => None,
        }
    }

    /// **The P15 check.**
    ///
    /// ⚠ **Cheap enough to run on every edit.** A check a developer has to ask for is a check they
    /// ask for after the mistake; the editor runs this while the graph is being drawn, which is only
    /// possible because it costs a couple of graph walks.
    pub fn check(&self) -> Vec<Finding> {
        let mut out = Vec::new();
        let names: BTreeSet<&str> = self.states.iter().map(|s| s.name.as_str()).collect();

        for t in &self.transitions {
            for end in [&t.from, &t.to] {
                if !names.contains(end.as_str()) {
                    out.push(Finding::UnknownState {
                        transition: format!("{} → {}", t.from, t.to),
                        missing: end.clone(),
                    });
                }
            }
        }

        let marked = self.states.iter().filter(|s| s.initial).count();
        if marked != 1 {
            out.push(Finding::InitialUnclear { count: marked });
            // ⚠ Without a start there is nothing to be reachable *from*, so the rest of the check would
            // report every state as unreachable — noise on top of the one finding that matters.
            return out;
        }
        let initial = self.initial().expect("exactly one is marked").to_string();

        // Free edges are those anyone may take.
        let free: Vec<&Transition> = self
            .transitions
            .iter()
            .filter(|t| t.requires.is_empty())
            .collect();

        let ungated_from = |start: &str| -> BTreeSet<String> {
            let mut seen: BTreeSet<String> = BTreeSet::new();
            let mut queue = vec![start.to_string()];
            while let Some(at) = queue.pop() {
                if !seen.insert(at.clone()) {
                    continue;
                }
                for t in free.iter().filter(|t| t.from == at) {
                    queue.push(t.to.clone());
                }
            }
            seen
        };

        // Reachability uses *every* edge: a state a player can only reach by holding something is still
        // reachable, and calling it unreachable would be wrong.
        let mut accessible: BTreeSet<String> = BTreeSet::new();
        let mut queue = vec![initial.clone()];
        while let Some(at) = queue.pop() {
            if !accessible.insert(at.clone()) {
                continue;
            }
            for t in self.transitions.iter().filter(|t| t.from == at) {
                queue.push(t.to.clone());
            }
        }

        for s in &self.states {
            if !accessible.contains(&s.name) {
                out.push(Finding::Inaccessible {
                    state: s.name.clone(),
                });
                continue;
            }
            if s.name == initial {
                continue;
            }

            let exits: Vec<&Transition> = self
                .transitions
                .iter()
                .filter(|t| t.from == s.name)
                .collect();
            if exits.is_empty() {
                out.push(Finding::DeadEnd {
                    state: s.name.clone(),
                });
                continue;
            }

            // ⚠ **Re-enterability, not reachability.** Can the initial state be regained *without*
            // holding anything? If not, every way back is gated, and that is the scenario's warning.
            if ungated_from(&s.name).contains(&initial) {
                continue;
            }
            let mut requires: Vec<String> = exits
                .iter()
                .flat_map(|t| t.requires.clone())
                .collect::<BTreeSet<String>>()
                .into_iter()
                .collect();
            requires.sort();
            if requires.is_empty() {
                // Every exit is free, yet the start is unreachable — a component that closes on itself.
                out.push(Finding::DeadEnd {
                    state: s.name.clone(),
                });
                continue;
            }
            out.push(Finding::ExitGated {
                state: s.name.clone(),
                requires,
                paths_affected: exits.len(),
            });
        }

        out.sort_by_key(|f| f.to_string());
        out
    }

    /// Does the graph satisfy P15?
    ///
    /// ⚠ **A gated exit does not fail it** — see [`Finding::blocks`]. Gating a way back is a design
    /// decision, and a check that blocked on it is a check somebody turns off.
    pub fn satisfies_p15(&self) -> bool {
        !self.check().iter().any(Finding::blocks)
    }

    /// How many transitions leave each state.
    ///
    /// ⚠ **A property of the graph, not of any view of it.** The editor draws it; a host that
    /// wants to know how constrained a state is reads the same number.
    pub fn out_degree(&self) -> BTreeMap<&str, usize> {
        let mut out: BTreeMap<&str, usize> =
            self.states.iter().map(|s| (s.name.as_str(), 0)).collect();
        for t in &self.transitions {
            if let Some(n) = out.get_mut(t.from.as_str()) {
                *n += 1;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `state-puzzle` §1.1 — the door latch, which satisfies P15.
    fn door_latch() -> StateGraph {
        StateGraph::new("door_latch")
            .state("closed", true)
            .state("open", false)
            .transition("closed", "open", &[], "plate")
            .transition("open", "closed", &[], "plate")
    }

    /// `08-graph-resources.md` §9 — the water level, whose `high` needs Iron Boots to leave.
    fn water_level() -> StateGraph {
        StateGraph::new("water_level")
            .state("low", true)
            .state("mid", false)
            .state("high", false)
            .transition("low", "mid", &[], "plaque")
            .transition("mid", "low", &[], "plaque")
            .transition("mid", "high", &[], "plaque")
            .transition("high", "mid", &["Iron Boots"], "plaque")
    }

    #[test]
    fn the_scenarios_own_graph_satisfies_p15() {
        let g = door_latch();
        assert!(g.check().is_empty(), "{:?}", g.check());
        assert!(g.satisfies_p15());
        assert!(
            g.check().is_empty(),
            "nothing to report is the passing shape"
        );
    }

    #[test]
    fn the_p15_check_fires_on_a_deliberately_broken_variant() {
        // ⚠ M19's green condition. Remove the way back and the drawing says so.
        let broken = StateGraph::new("door_latch")
            .state("closed", true)
            .state("open", false)
            .transition("closed", "open", &[], "plate");

        let findings = broken.check();
        assert_eq!(
            findings,
            vec![Finding::DeadEnd {
                state: "open".into()
            }]
        );
        assert!(!broken.satisfies_p15());
        assert!(findings[0].to_string().contains("strands the variable"));
    }

    #[test]
    fn a_gated_way_back_is_the_warning_the_design_draws() {
        // ⚠ "`high` is accessible but `low` is not accessible FROM `high` without [Iron Boots]".
        let findings = water_level().check();
        assert_eq!(
            findings,
            vec![Finding::ExitGated {
                state: "high".into(),
                requires: vec!["Iron Boots".into()],
                paths_affected: 1,
            }]
        );
        let line = findings[0].to_string();
        assert!(line.contains("Iron Boots"));
        assert!(line.contains("potential softlock"));
        assert!(line.contains("1 path(s) affected"));
    }

    #[test]
    fn a_gated_exit_warns_without_blocking_because_it_is_a_design_decision() {
        // ⚠ A check that blocked on it is a check somebody turns off.
        let g = water_level();
        assert!(!g.check().is_empty());
        assert!(g.satisfies_p15(), "a gated way back is legitimate");
        assert!(!g.check()[0].blocks());
    }

    #[test]
    fn re_enterability_is_the_question_rather_than_accessibility() {
        // ⚠ Every state reachable and still a softlock: `mid` can be entered, and leaving it toward the
        // start needs a key.
        let g = StateGraph::new("v")
            .state("a", true)
            .state("b", false)
            .transition("a", "b", &[], "x")
            .transition("b", "a", &["Key"], "x");
        assert!(
            g.check()
                .iter()
                .any(|f| matches!(f, Finding::ExitGated { .. })),
            "accessibility alone would have called this fine"
        );
    }

    #[test]
    fn a_state_nothing_reaches_is_a_different_finding_from_one_nobody_leaves() {
        // ⚠ Telling a developer the wrong one sends them to the wrong end of the graph.
        let g = StateGraph::new("v")
            .state("a", true)
            .state("orphan", false)
            .transition("a", "a", &[], "x");
        assert_eq!(
            g.check(),
            vec![Finding::Inaccessible {
                state: "orphan".into()
            }]
        );
        assert!(g.check()[0].blocks());
    }

    #[test]
    fn a_control_that_is_itself_state_gated_shows_as_a_cycle_that_does_not_close() {
        // ⚠ The hard case nobody catches by reading: the way back exists and needs what only the far
        // state grants.
        let g = StateGraph::new("v")
            .state("start", true)
            .state("locked", false)
            .transition("start", "locked", &[], "lever")
            .transition("locked", "start", &["OnlyFoundBeyondLocked"], "lever");
        let findings = g.check();
        assert_eq!(
            findings,
            vec![Finding::ExitGated {
                state: "locked".into(),
                requires: vec!["OnlyFoundBeyondLocked".into()],
                paths_affected: 1,
            }]
        );
    }

    #[test]
    fn no_initial_state_or_two_is_one_finding_and_stops_the_rest() {
        // ⚠ Without a start there is nothing to be reachable from, and reporting every state as
        // unreachable would bury the one finding that matters.
        let none = StateGraph::new("v").state("a", false).state("b", false);
        assert_eq!(none.check(), vec![Finding::InitialUnclear { count: 0 }]);

        let two = StateGraph::new("v").state("a", true).state("b", true);
        assert_eq!(two.check(), vec![Finding::InitialUnclear { count: 2 }]);
        assert!(two.check()[0].blocks());
    }

    #[test]
    fn a_transition_naming_a_state_that_does_not_exist_is_named() {
        let g = StateGraph::new("v")
            .state("a", true)
            .transition("a", "ghost", &[], "x");
        assert!(g
            .check()
            .iter()
            .any(|f| matches!(f, Finding::UnknownState { .. })));
    }

    #[test]
    fn a_long_free_path_home_counts_as_re_enterable() {
        // ⚠ Getting back need not be one hop — a three-state loop is a loop.
        let g = StateGraph::new("v")
            .state("a", true)
            .state("b", false)
            .state("c", false)
            .transition("a", "b", &[], "x")
            .transition("b", "c", &[], "x")
            .transition("c", "a", &[], "x");
        assert!(g.check().is_empty(), "{:?}", g.check());
    }

    #[test]
    fn a_finding_names_states_rather_than_indices() {
        // ⚠ The developer is looking at a picture with names on it.
        for f in water_level().check() {
            let text = f.to_string();
            assert!(text.contains('`'), "{text}");
            assert!(!text.contains("state["), "{text}");
        }
    }

    #[test]
    fn the_drawing_knows_how_many_ways_leave_each_state() {
        let graph = water_level();
        let degrees = graph.out_degree();
        assert_eq!(degrees["low"], 1);
        assert_eq!(degrees["mid"], 2);
        assert_eq!(degrees["high"], 1);
    }

    #[test]
    fn the_initial_state_is_never_reported_for_being_hard_to_leave() {
        // It is where the variable starts; "you cannot get back to the start from the start" is not a
        // finding.
        let g = StateGraph::new("v")
            .state("only", true)
            .transition("only", "only", &["Key"], "x");
        assert!(g.check().is_empty(), "{:?}", g.check());
    }
}
