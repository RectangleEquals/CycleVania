//! **`GUARDED` verification** — actively proving no alternative route past a gate exists.
//!
//! [`SkipPolicy`](crate::gate::SkipPolicy) has three values and, until now, two behaviours. `TOLERATED`
//! and `EXACT` are cheap and specified; `GUARDED` said *"actively verify no alternative exists at that
//! sphere"* and named **no search, no cost model, and no answer for *cannot prove absence***. So the
//! enum was authorable and one of its values did nothing.
//!
//! # What the search is
//!
//! The claim to prove is narrow, which is what makes it affordable:
//!
//! > with the capabilities of the sphere the gate sits in, **minus what the gate itself demands**, the
//! > far side is not reachable from the near side.
//!
//! That is one [`sweep_from`](crate::mission::MissionGraph::sweep_from) over the mission graph with a
//! reduced holding set. No new machinery, no geometry, no path enumeration — the same fixed point the
//! solver already computes, run once against a smaller premise.
//!
//! ⚠ **Bounded to the gate's own sphere, deliberately.** Verifying against *every* sphere would ask
//! whether the gate is skippable by a player who has already finished the game, which is true of almost
//! every gate and interesting about none of them. The question a designer means by *"this one is
//! sacred"* is **can it be skipped when it is met**.
//!
//! # What it costs
//!
//! One sweep per guarded gate per verification round: `O(V + E)` over the mission graph, which is the
//! small graph — tens of nodes, not the geometry. **That is why the design can say `GUARDED` is
//! expensive and rare in the same breath**: it is expensive *relative to `TOLERATED`, which is free*,
//! and a designer marks two or three gates rather than two hundred.
//!
//! ⚠ **The budget is declared and enforced** rather than trusted. A project that marks two hundred
//! gates `GUARDED` gets a [`VerifyError::BudgetExhausted`] naming the count, not a generator that
//! quietly takes a hundred times longer. A cost model nobody can exceed is a cost model nobody has to
//! think about, and this one is exceedable on purpose.
//!
//! # What happens when it cannot prove absence
//!
//! ⚠ **A loud failure, never a silent pass.** The reachability sweep is over the *mission* graph, whose
//! edges are `Trivalent` once geometry starts answering: an edge can be **`AMBIGUOUS`** — inside the
//! tolerance band, undecided until the next fidelity rung. A sweep that meets one cannot conclude
//! *"unreachable"*, and reporting `Exclusive` anyway would be the generator asserting a proof it does
//! not have about the one gate the designer marked sacred.
//!
//! So the third verdict is [`Verification::Unproven`], and it is **not** a failure to place. It is an
//! escalation: *"stop asking me at this fidelity; ask the layer that can resolve the band."* That is
//! the same P6 rule the attempt budget follows — reaching a limit escalates, never abandons — and it is
//! why this module reports rather than decides.

use crate::arena::Handle;
use crate::gate::SkipPolicy;
use crate::mission::LocationId;
use crate::mission::{MissionGraph, Rule};
use crate::node::Node;
use crate::object::ObjectId;
use crate::trivalent::Trivalent;
use crate::unlock::GrantMap;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// How many guarded sweeps one verification pass may run.
///
/// ⚠ **Declared here rather than left to the caller.** *"Expensive and should be rare"* is a sentence
/// in a design document until something counts, and the count is what turns it into a property a
/// project either has or does not.
pub const DEFAULT_SWEEP_BUDGET: u32 = 16;

/// What a guarded gate's verification concluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verification {
    /// No alternative route exists at this sphere. The gate holds.
    Exclusive,
    /// An alternative route exists, and here is where it lands.
    ///
    /// ⚠ **The breach names a scope**, because *"a skip exists"* is not actionable and *"wing B's far
    /// side is reachable from the balcony"* is.
    Breached { via: Handle<Node> },
    /// ⚠ **The sweep met an undecided edge and cannot conclude.**
    ///
    /// Not a pass and not a failure — an **escalation**. Something at a higher fidelity has to settle
    /// the band before this question has an answer.
    Unproven { undecided: usize },
}

impl Verification {
    /// Did the gate hold?
    ///
    /// ⚠ **`Unproven` answers `false`**, and that is the whole point. A verification that cannot prove
    /// absence has not proven it, and the conservative reading is the only one that keeps *"the
    /// designer marked this sacred"* meaningful.
    pub fn holds(&self) -> bool {
        matches!(self, Verification::Exclusive)
    }

    /// Does this need a layer with more information than the caller has?
    pub fn escalates(&self) -> bool {
        matches!(self, Verification::Unproven { .. })
    }

    /// As a three-valued answer to *"is this gate exclusive?"*.
    pub fn as_trivalent(&self) -> Trivalent {
        match self {
            Verification::Exclusive => Trivalent::Yes,
            Verification::Breached { .. } => Trivalent::No,
            Verification::Unproven { .. } => Trivalent::Ambiguous,
        }
    }
}

impl fmt::Display for Verification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verification::Exclusive => write!(f, "exclusive — no alternative route at this sphere"),
            Verification::Breached { .. } => write!(f, "breached — an alternative route exists"),
            Verification::Unproven { undecided } => write!(
                f,
                "unproven — {undecided} undecided edge(s); absence cannot be established here"
            ),
        }
    }
}

/// Why a verification pass could not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// More guarded gates than the sweep budget allows.
    BudgetExhausted { guarded: u32, budget: u32 },
    /// The named edge is not in the graph.
    NoSuchEdge { index: usize },
    /// The edge carries no gate, so there is nothing to be exclusive about.
    NotGated { index: usize },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::BudgetExhausted { guarded, budget } => write!(
                f,
                "{guarded} GUARDED gates exceeds the sweep budget of {budget}; \
                 GUARDED is the expensive path and is meant to be rare"
            ),
            VerifyError::NoSuchEdge { index } => write!(f, "no edge at index {index}"),
            VerifyError::NotGated { index } => {
                write!(f, "the edge at index {index} has no gate to verify")
            }
        }
    }
}

impl std::error::Error for VerifyError {}

/// Runs the exclusivity proof for guarded gates.
///
/// ⚠ **It reports and never repairs.** Deciding what to do about a breach — nudge the geometry, refuse
/// an adoption, escalate — belongs to the caller, because the same verdict means different things at
/// different layers. A verifier that also acted would make that choice once, invisibly, for everyone.
pub struct Verifier<'a> {
    graph: &'a MissionGraph,
    budget: u32,
    /// Edges whose traversability geometry has not settled.
    ///
    /// ⚠ **Carried explicitly rather than inferred.** The mission graph's rules are decidable; what is
    /// *not* decidable is whether a physical edge is crossable while the hull is still a proxy. Only
    /// the caller knows which those are, so an empty set here means *"everything is decided"* and says
    /// so, rather than pretending.
    undecided: BTreeSet<usize>,
}

impl<'a> Verifier<'a> {
    /// A verifier over this graph, with the default sweep budget.
    pub fn new(graph: &'a MissionGraph) -> Self {
        Verifier {
            graph,
            budget: DEFAULT_SWEEP_BUDGET,
            undecided: BTreeSet::new(),
        }
    }

    /// Change how many sweeps a pass may run.
    pub fn with_budget(mut self, budget: u32) -> Self {
        self.budget = budget;
        self
    }

    /// Mark an edge as not yet settled by geometry.
    pub fn undecided(mut self, edge: usize) -> Self {
        self.undecided.insert(edge);
        self
    }

    /// Verify one guarded gate.
    ///
    /// The premise is the sphere's capabilities **minus what this gate demands**: the state a player is
    /// in when they first meet the lock.
    pub fn verify(
        &self,
        edge: usize,
        held_at_sphere: &BTreeSet<ObjectId>,
        placements: &BTreeMap<LocationId, ObjectId>,
        grants: &GrantMap,
    ) -> Result<Verification, VerifyError> {
        let e = self
            .graph
            .edges()
            .get(edge)
            .ok_or(VerifyError::NoSuchEdge { index: edge })?;
        if !e.is_gated() {
            return Err(VerifyError::NotGated { index: edge });
        }

        // The premise: everything the sphere affords, except what this gate asks for.
        let demanded = e.rule.unlocks();
        let without: BTreeSet<ObjectId> = held_at_sphere.difference(&demanded).copied().collect();

        // ⚠ **An undecided edge is not walked.** Treating it as crossable would let the verifier
        // report a breach it cannot demonstrate; treating it as absent is the conservative half, and
        // the frontier check below is what stops that half from becoming a silent pass.
        let decided = self.decided_graph(e.from);
        let reach = decided.sweep_from(e.from, &without, placements, grants);

        if reach.accessible(e.to) {
            return Ok(Verification::Breached { via: e.to });
        }

        // ⚠ **Absence is proven only when nothing undecided touches the frontier.** An edge whose near
        // side the sweep reached and whose far side it did not is exactly the edge that could carry a
        // route once geometry settles — so the sweep stopped for a reason that might dissolve at the
        // next fidelity rung, and *"unreachable"* is not a conclusion this layer is entitled to.
        let undecided = self
            .undecided
            .iter()
            .filter(|i| {
                self.graph
                    .edges()
                    .get(**i)
                    .is_some_and(|u| reach.accessible(u.from) && !reach.accessible(u.to))
            })
            .count();
        if undecided > 0 {
            return Ok(Verification::Unproven { undecided });
        }

        Ok(Verification::Exclusive)
    }

    /// The graph with undecided edges removed, so the sweep walks only what is known crossable.
    fn decided_graph(&self, origin: Handle<Node>) -> MissionGraph {
        if self.undecided.is_empty() {
            return self.graph.clone();
        }
        let mut out = MissionGraph::new(origin);
        for (i, e) in self.graph.edges().iter().enumerate() {
            if !self.undecided.contains(&i) {
                out.add_edge(e.clone());
            }
        }
        for (id, loc) in self.graph.locations() {
            out.add_location(id, loc);
        }
        out
    }

    /// Verify every guarded gate in the graph, refusing to start if the budget cannot cover it.
    ///
    /// ⚠ **The budget is checked before the first sweep, not counted down during.** A pass that ran
    /// fifteen sweeps and then reported exhaustion would have spent the cost and produced a partial
    /// answer, which is the worst of both.
    pub fn verify_all(
        &self,
        policies: &BTreeMap<usize, SkipPolicy>,
        held_at_sphere: &BTreeSet<ObjectId>,
        placements: &BTreeMap<LocationId, ObjectId>,
        grants: &GrantMap,
    ) -> Result<BTreeMap<usize, Verification>, VerifyError> {
        let guarded: Vec<usize> = policies
            .iter()
            .filter(|(_, p)| p.requires_search())
            .map(|(i, _)| *i)
            .collect();
        if guarded.len() as u32 > self.budget {
            return Err(VerifyError::BudgetExhausted {
                guarded: guarded.len() as u32,
                budget: self.budget,
            });
        }
        let mut out = BTreeMap::new();
        for i in guarded {
            out.insert(i, self.verify(i, held_at_sphere, placements, grants)?);
        }
        Ok(out)
    }
}

/// The gate demands nothing the sphere does not already have — so it cannot be exclusive.
///
/// ⚠ **A gate whose rule is satisfied by the sphere it sits in is open, not guarded.** Marking it
/// `GUARDED` is a mistake a designer makes when the rule and the sphere drift apart, and verification
/// would report `Breached` for a reason that has nothing to do with a skip. Naming the case separately
/// is what turns a confusing result into an actionable one.
pub fn gate_is_vacuous(rule: &Rule, held_at_sphere: &BTreeSet<ObjectId>) -> bool {
    rule.is_open() || rule.is_satisfied(held_at_sphere)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::MissionEdge;
    use crate::node::NodeGraph;

    fn oid(s: &str) -> ObjectId {
        ObjectId::derived("unlock", s)
    }

    /// `main_plaza -> wing_b`, gated on Missiles, with `wing_a` beside it and one scope joined to
    /// nothing.
    ///
    /// ⚠ **Every scope comes from the same `NodeGraph`.** A handle is an index into one arena, so a
    /// scope built in a second graph silently collides with a scope in the first - which is how an
    /// "unreachable" fixture turned into `main_plaza` and made the frontier test below pass for the
    /// wrong reason.
    struct Wings {
        m: MissionGraph,
        plaza: Handle<Node>,
        a: Handle<Node>,
        b: Handle<Node>,
        loose: Handle<Node>,
    }

    fn wings() -> Wings {
        let mut g = NodeGraph::new(1.0, 1);
        let area = g.add_child(g.root(), "area").unwrap();
        let plaza = g.add_child(area, "main_plaza").unwrap();
        let a = g.add_child(area, "wing_a").unwrap();
        let b = g.add_child(area, "wing_b").unwrap();
        let loose = g.add_child(area, "sealed_vault").unwrap();
        let mut m = MissionGraph::new(plaza);
        m.add_edge(MissionEdge::open(plaza, a));
        m.add_edge(MissionEdge::gated(plaza, b, Rule::has(oid("Missiles"))));
        Wings {
            m,
            plaza,
            a,
            b,
            loose,
        }
    }

    #[test]
    fn a_gate_with_no_way_around_it_is_exclusive() {
        let w = wings();
        let got = Verifier::new(&w.m)
            .verify(1, &BTreeSet::new(), &BTreeMap::new(), &GrantMap::new())
            .unwrap();
        assert_eq!(got, Verification::Exclusive);
        assert!(got.holds());
        assert_eq!(got.as_trivalent(), Trivalent::Yes);
    }

    #[test]
    fn a_route_around_the_gate_is_a_breach_that_names_where_it_lands() {
        // The discovered ledge: wing_a reaches wing_b without Missiles.
        let mut w = wings();
        w.m.add_edge(MissionEdge::open(w.a, w.b));
        let got = Verifier::new(&w.m)
            .verify(1, &BTreeSet::new(), &BTreeMap::new(), &GrantMap::new())
            .unwrap();
        assert_eq!(got, Verification::Breached { via: w.b });
        assert!(!got.holds());
        assert_eq!(got.as_trivalent(), Trivalent::No);
    }

    #[test]
    fn the_premise_subtracts_what_the_gate_itself_demands() {
        // ⚠ Holding Missiles at this sphere must not make the gate look skippable: the question
        // is whether it can be passed *without* what it asks for.
        let w = wings();
        let held: BTreeSet<ObjectId> = [oid("Missiles")].into_iter().collect();
        let got = Verifier::new(&w.m)
            .verify(1, &held, &BTreeMap::new(), &GrantMap::new())
            .unwrap();
        assert_eq!(
            got,
            Verification::Exclusive,
            "the gate own unlock is subtracted from the premise, or every gate verifies as breached"
        );
    }

    #[test]
    fn an_undecided_edge_on_the_frontier_makes_absence_unprovable() {
        // wing_a touches a scope geometry has not settled a route into. The sweep stops at the
        // frontier for a reason that might dissolve at the next rung, so absence is not established.
        let mut w = wings();
        w.m.add_edge(MissionEdge::open(w.a, w.loose));
        let j = w.m.edges().len() - 1;
        let got = Verifier::new(&w.m)
            .undecided(j)
            .verify(1, &BTreeSet::new(), &BTreeMap::new(), &GrantMap::new())
            .unwrap();
        assert_eq!(got, Verification::Unproven { undecided: 1 });
        assert!(!got.holds(), "unproven is not a pass");
        assert!(got.escalates());
        assert_eq!(got.as_trivalent(), Trivalent::Ambiguous);
    }

    #[test]
    fn an_undecided_edge_nowhere_near_the_frontier_does_not_block_the_proof() {
        // ⚠ Otherwise one unsettled edge anywhere in the world would make every gate unprovable,
        // and `GUARDED` would report `Unproven` forever.
        let mut w = wings();
        w.m.add_edge(MissionEdge::open(w.loose, w.plaza));
        let j = w.m.edges().len() - 1;
        let got = Verifier::new(&w.m)
            .undecided(j)
            .verify(1, &BTreeSet::new(), &BTreeMap::new(), &GrantMap::new())
            .unwrap();
        assert_eq!(got, Verification::Exclusive);
    }

    #[test]
    fn an_ungated_edge_has_nothing_to_verify() {
        let w = wings();
        assert_eq!(
            Verifier::new(&w.m).verify(0, &BTreeSet::new(), &BTreeMap::new(), &GrantMap::new()),
            Err(VerifyError::NotGated { index: 0 })
        );
    }

    #[test]
    fn the_budget_refuses_before_it_spends_rather_than_partway_through() {
        let w = wings();
        let policies: BTreeMap<usize, SkipPolicy> =
            [(1, SkipPolicy::Guarded)].into_iter().collect();
        let err = Verifier::new(&w.m)
            .with_budget(0)
            .verify_all(
                &policies,
                &BTreeSet::new(),
                &BTreeMap::new(),
                &GrantMap::new(),
            )
            .unwrap_err();
        assert_eq!(
            err,
            VerifyError::BudgetExhausted {
                guarded: 1,
                budget: 0
            }
        );
        assert!(err.to_string().contains("expensive path"));
    }

    #[test]
    fn tolerated_runs_no_sweep_and_the_other_two_do() {
        // ⚠ **`EXACT` searches too** - it *reports* every alternative it finds. The difference
        // between it and `GUARDED` is what a found alternative *means*, not whether anyone looked:
        // only `GUARDED` refuses. A budget that skipped `EXACT` would under-count the cost of a
        // project that marks fifty gates for reporting.
        let w = wings();
        let only_tolerated: BTreeMap<usize, SkipPolicy> =
            [(1, SkipPolicy::Tolerated)].into_iter().collect();
        assert!(
            Verifier::new(&w.m)
                .verify_all(
                    &only_tolerated,
                    &BTreeSet::new(),
                    &BTreeMap::new(),
                    &GrantMap::new()
                )
                .unwrap()
                .is_empty(),
            "TOLERATED is the free default; nothing is swept for it"
        );

        for p in [SkipPolicy::Exact, SkipPolicy::Guarded] {
            let policies: BTreeMap<usize, SkipPolicy> = [(1, p)].into_iter().collect();
            let got = Verifier::new(&w.m)
                .verify_all(
                    &policies,
                    &BTreeSet::new(),
                    &BTreeMap::new(),
                    &GrantMap::new(),
                )
                .unwrap();
            assert_eq!(got.len(), 1, "{p} pays for a search");
            assert_eq!(got[&1], Verification::Exclusive);
        }
        assert!(SkipPolicy::Guarded.refuses_alternatives());
        assert!(
            !SkipPolicy::Exact.refuses_alternatives(),
            "EXACT reports what it finds; it does not refuse it"
        );
    }

    #[test]
    fn a_gate_the_sphere_already_satisfies_is_vacuous_rather_than_breached() {
        let held: BTreeSet<ObjectId> = [oid("Missiles")].into_iter().collect();
        assert!(gate_is_vacuous(&Rule::has(oid("Missiles")), &held));
        assert!(gate_is_vacuous(&Rule::Always, &BTreeSet::new()));
        assert!(!gate_is_vacuous(
            &Rule::has(oid("Missiles")),
            &BTreeSet::new()
        ));
    }
}
