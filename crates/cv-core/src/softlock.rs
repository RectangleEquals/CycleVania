//! **The un-softlockable guarantee** — the property that separates a generator you can ship from one
//! you cannot.
//!
//! # Solvable is not enough
//!
//! M09 guarantees a world is **solvable**: a path to the goal exists. That is the standard bar, and it
//! is not sufficient, because it only describes the *optimal* player. Consider:
//!
//! ```text
//! start ──▶ vault ──(one-way drop)──▶ depths ──▶ goal(needs the key)
//!             │
//!             └── the key is here
//! ```
//!
//! Solvable? Yes — take the key, then drop. But a player who drops *first* is stranded forever, and
//! the generator cheerfully shipped that world. No amount of solvability checking catches it, because
//! solvability asks "does a path exist?" and the answer is still yes.
//!
//! The stronger property is:
//!
//! > **From every state a player can reach, the goal must remain reachable** — or a recovery
//! > affordance must return them.
//!
//! That quantifies over *all* reachable states rather than one path, which is why it needs a different
//! analysis rather than a stricter version of the same one.
//!
//! # Where the danger actually lives
//!
//! Capabilities are monotone — you gain them and never lose them — so more progress is never worse.
//! That means a player cannot strand themselves by *collecting*; only by **committing**. The commit
//! points are:
//!
//! * **one-way transitions** — a drop you cannot climb back up, a one-way shortcut, a door that seals;
//! * (later) **consumables**, which break monotonicity and are noted as a GAP below.
//!
//! So the analysis is: for every one-way edge, and every capability set a player could plausibly hold
//! when crossing it, is the goal still reachable from the far side?
//!
//! # Why this is tractable
//!
//! "Every capability set" sounds exponential, and in the abstract it is — `2^n` over capabilities. Two
//! things rescue it:
//!
//! 1. **`n` is small.** Progression capabilities in a metroidvania number in the handful; MP1 has
//!    roughly a dozen. `2^12` is four thousand, and each check is a graph traversal over a few hundred
//!    nodes.
//! 2. **Safety is monotone, so it prunes.** If a player is safe crossing with set `S`, they are safe
//!    with any superset — more capabilities cannot reduce reachability. So once `S` is proven safe,
//!    every superset of it is skipped. In practice most of the lattice disappears.
//!
//! The analysis reports its own cost ([`SoftlockAnalysis::states_examined`]) and refuses rather than
//! stalls when a world exceeds [`SoftlockAnalyzer::max_capabilities`] — a bounded honest failure beats
//! an unbounded wait.
//!
//! # Conservatism
//!
//! Where the analysis is unsure it errs toward **reporting** a hazard. A false positive costs a
//! needlessly-safe world; a false negative ships a soft-locked one. For a safety property that trade is
//! not close.

use crate::mission::{LocationId, MissionGraph};
use crate::node::Node;
use crate::object::ObjectId;
use crate::Handle;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// What kind of trap was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftlockKind {
    /// After the commit, the goal can no longer be reached at all.
    GoalUnreachable,
    /// The goal is technically unreachable *and* no recovery point is available either — the same
    /// condition, distinguished for the diagnostic because the fix differs (a route vs. a warp).
    NoRecovery,
}

impl fmt::Display for SoftlockKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SoftlockKind::GoalUnreachable => "goal unreachable",
            SoftlockKind::NoRecovery => "no recovery available",
        })
    }
}

/// A concrete way a player can strand themselves.
///
/// Deliberately carries the **exact capability set** that traps them, not just "this edge is risky".
/// A hazard a dev cannot reproduce is a hazard they will not fix.
#[derive(Clone, Debug, PartialEq)]
pub struct Softlock {
    /// Index of the one-way edge that commits the player.
    pub edge: usize,
    /// Where they cross from.
    pub from: Handle<Node>,
    /// Where they land.
    pub to: Handle<Node>,
    /// The capabilities they hold at the moment of crossing — the reproduction case.
    pub holding: BTreeSet<ObjectId>,
    /// What kind of trap.
    pub kind: SoftlockKind,
}

impl fmt::Display for Softlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "one-way edge {} strands a player holding {} capabilit{}: {}",
            self.edge,
            self.holding.len(),
            if self.holding.len() == 1 { "y" } else { "ies" },
            self.kind
        )
    }
}

/// Why the analysis could not be completed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnalysisLimit {
    /// More capabilities than the configured bound, so the state space was not enumerated.
    TooManyCapabilities { found: usize, limit: usize },
    /// The graph declares no goal, so "the goal stays reachable" has no meaning.
    NoGoal,
}

impl fmt::Display for AnalysisLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalysisLimit::TooManyCapabilities { found, limit } => write!(
                f,
                "{found} capabilities exceeds the analysis bound of {limit}; \
                 raise SoftlockAnalyzer::max_capabilities or reduce progression items"
            ),
            AnalysisLimit::NoGoal => {
                write!(
                    f,
                    "the mission graph declares no goal, so softlocks are undefined"
                )
            }
        }
    }
}

/// The result of a reachability-preservation pass.
#[derive(Clone, Debug, PartialEq)]
pub struct SoftlockAnalysis {
    /// Every way a player can strand themselves. Empty means the world is un-softlockable.
    pub hazards: Vec<Softlock>,
    /// How many one-way commits were examined.
    pub commits_checked: usize,
    /// Capability sets evaluated — the cost model. Reported so the bound can be tuned against real
    /// worlds rather than guessed.
    pub states_examined: usize,
    /// Set when the analysis could not run to completion.
    pub limit: Option<AnalysisLimit>,
}

impl SoftlockAnalysis {
    /// Did the analysis complete *and* find nothing?
    ///
    /// Note that an incomplete analysis is **not** safe — an unanswered question is not a "no".
    pub fn is_un_softlockable(&self) -> bool {
        self.limit.is_none() && self.hazards.is_empty()
    }

    /// The one-way edges implicated, deduplicated.
    pub fn hazardous_edges(&self) -> BTreeSet<usize> {
        self.hazards.iter().map(|h| h.edge).collect()
    }

    /// Remove every hazard, reporting what was changed.
    ///
    /// The repair is to make each offending edge two-way, which always works: a commit you can undo is
    /// not a commit. It is deliberately the blunt fix — richer options (placing a recovery affordance,
    /// relocating the stranded item) preserve the one-way *feel* and belong with the legibility work at
    /// M31, where there is a reasoner able to choose between them.
    ///
    /// Lives on the analysis rather than the analyzer so the mission can be borrowed mutably: the
    /// finding is data, and applying it needs no further reading.
    pub fn repair(&self, mission: &mut MissionGraph) -> Vec<Repair> {
        self.hazardous_edges()
            .into_iter()
            .filter(|edge| mission.make_reversible(*edge))
            .map(|edge| Repair::MadeReversible { edge })
            .collect()
    }
}

/// A change made to remove a hazard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Repair {
    /// The one-way edge was made two-way — the player can climb back out.
    MadeReversible { edge: usize },
    /// The landing scope was marked a recovery point.
    AddedRecovery { scope: Handle<Node> },
}

impl fmt::Display for Repair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Repair::MadeReversible { edge } => write!(f, "made edge {edge} reversible"),
            Repair::AddedRecovery { scope } => write!(f, "added a recovery point at {scope:?}"),
        }
    }
}

/// Runs the reachability-preservation pass.
pub struct SoftlockAnalyzer<'a> {
    mission: &'a MissionGraph,
    grants: &'a BTreeMap<ObjectId, ObjectId>,
    placements: &'a BTreeMap<LocationId, ObjectId>,
    initial: BTreeSet<ObjectId>,
    /// Above this many distinct capabilities the pass declines rather than enumerating `2^n`.
    max_capabilities: usize,
}

impl<'a> SoftlockAnalyzer<'a> {
    /// The default capability bound. `2^14` sets is a few seconds at worst and covers every real
    /// metroidvania; beyond it, declining loudly is better than stalling.
    pub const DEFAULT_MAX_CAPABILITIES: usize = 14;

    /// An analyzer over a solved world.
    pub fn new(
        mission: &'a MissionGraph,
        placements: &'a BTreeMap<LocationId, ObjectId>,
        grants: &'a BTreeMap<ObjectId, ObjectId>,
    ) -> Self {
        SoftlockAnalyzer {
            mission,
            grants,
            placements,
            initial: BTreeSet::new(),
            max_capabilities: Self::DEFAULT_MAX_CAPABILITIES,
        }
    }

    /// Capabilities the player starts with.
    pub fn with_initial(mut self, capabilities: impl IntoIterator<Item = ObjectId>) -> Self {
        self.initial.extend(capabilities);
        self
    }

    /// Raise or lower the enumeration bound.
    pub fn with_max_capabilities(mut self, max: usize) -> Self {
        self.max_capabilities = max;
        self
    }

    /// Run the pass.
    pub fn analyze(&self) -> SoftlockAnalysis {
        let mut analysis = SoftlockAnalysis {
            hazards: Vec::new(),
            commits_checked: 0,
            states_examined: 0,
            limit: None,
        };

        let Some(goal) = self.mission.goal() else {
            analysis.limit = Some(AnalysisLimit::NoGoal);
            return analysis;
        };

        // Only capabilities actually obtainable in this world matter; a registered-but-unplaced item
        // cannot be part of any state a player reaches.
        let full = self
            .mission
            .sweep(&self.initial, self.placements, self.grants);
        let universe: Vec<ObjectId> = full.held.iter().copied().collect();

        if universe.len() > self.max_capabilities {
            analysis.limit = Some(AnalysisLimit::TooManyCapabilities {
                found: universe.len(),
                limit: self.max_capabilities,
            });
            return analysis;
        }

        // One-way edges are the only commit points, because capabilities are monotone: collecting
        // never hurts, so a player can only trap themselves by moving somewhere they cannot leave.
        let one_ways: Vec<(usize, Handle<Node>, Handle<Node>)> = self
            .mission
            .edges()
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.reversible)
            .map(|(i, e)| (i, e.from, e.to))
            .collect();

        for (edge_index, from, to) in one_ways {
            analysis.commits_checked += 1;
            let rule = &self.mission.edges()[edge_index].rule;

            // Safety is monotone in capabilities, so a superset of a safe set is safe. Enumerating in
            // increasing size lets each proven-safe set prune everything above it.
            let mut safe: Vec<BTreeSet<ObjectId>> = Vec::new();

            for size in 0..=universe.len() {
                for combo in combinations(&universe, size) {
                    let held: BTreeSet<ObjectId> = self
                        .initial
                        .iter()
                        .copied()
                        .chain(combo.iter().copied())
                        .collect();

                    if safe.iter().any(|s| s.is_subset(&held)) {
                        continue; // a safe subset already covers this state
                    }
                    if !self.is_achievable(&held) || !rule.is_satisfied(&held) {
                        continue;
                    }
                    if !self.mission.traverse(&held).contains(&from) {
                        continue; // cannot even get to the crossing with only these
                    }

                    analysis.states_examined += 1;

                    // Standing on the far side, holding exactly this.
                    let after = self
                        .mission
                        .sweep_from(to, &held, self.placements, self.grants);

                    let reaches_goal = after.reaches(goal);
                    let reaches_recovery = self
                        .mission
                        .recovery_points()
                        .iter()
                        .any(|r| after.reaches(*r));

                    if reaches_goal || reaches_recovery {
                        safe.push(held);
                    } else {
                        analysis.hazards.push(Softlock {
                            edge: edge_index,
                            from,
                            to,
                            holding: held,
                            kind: if self.mission.recovery_points().is_empty() {
                                SoftlockKind::GoalUnreachable
                            } else {
                                SoftlockKind::NoRecovery
                            },
                        });
                    }
                }
            }
        }
        analysis
    }

    /// Could a player actually be holding exactly this set?
    ///
    /// A set is achievable when it is **self-supporting**: everything in it can be collected using
    /// only itself. That rules out states like "holds the late-game dash but nothing else" which no
    /// route produces, and so keeps the analysis from reporting traps that cannot occur.
    fn is_achievable(&self, held: &BTreeSet<ObjectId>) -> bool {
        let reachable = self.mission.traverse(held);
        let collectible: BTreeSet<ObjectId> = reachable
            .iter()
            .flat_map(|s| self.mission.locations_in(*s))
            .filter_map(|loc| self.placements.get(&loc))
            .filter_map(|item| self.grants.get(item))
            .copied()
            .chain(self.initial.iter().copied())
            .collect();
        held.is_subset(&collectible)
    }
}

/// Every `size`-element combination of `items`, in a deterministic order.
///
/// Index-based rather than value-based so the ordering depends only on the input sequence, never on
/// hashing or addresses.
fn combinations<T: Copy>(items: &[T], size: usize) -> Vec<Vec<T>> {
    if size == 0 {
        return vec![Vec::new()];
    }
    if size > items.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut indices: Vec<usize> = (0..size).collect();
    loop {
        out.push(indices.iter().map(|i| items[*i]).collect());
        // Advance to the next combination in lexicographic index order.
        let mut i = size;
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            if indices[i] != i + items.len() - size {
                break;
            }
            if i == 0 {
                return out;
            }
        }
        indices[i] += 1;
        for j in i + 1..size {
            indices[j] = indices[j - 1] + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::{Location, MissionEdge, Rule};
    use crate::node::{NodeGraph, NodeState};
    use cv_determinism::{Aabb, Vec3};

    fn cap(name: &str) -> ObjectId {
        ObjectId::derived("capability", name)
    }
    fn item(name: &str) -> ObjectId {
        ObjectId::derived("item", name)
    }

    /// `n` rooms in a chain, all realized.
    fn rooms(n: usize) -> (NodeGraph, Vec<Handle<Node>>) {
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "reach").unwrap();
        let area = g.add_child(reach, "area").unwrap();
        let rooms: Vec<Handle<Node>> = (0..n)
            .map(|i| g.add_child(area, format!("room_{i}")).unwrap())
            .collect();
        for h in g.walk() {
            g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
                .unwrap();
        }
        for h in g.walk() {
            g.advance(h, NodeState::Realized).unwrap();
        }
        (g, rooms)
    }

    /// The canonical trap: the key sits *before* a one-way drop, and the goal beyond it needs the key.
    fn trap_world() -> (
        NodeGraph,
        MissionGraph,
        BTreeMap<LocationId, ObjectId>,
        BTreeMap<ObjectId, ObjectId>,
    ) {
        let (g, r) = rooms(4);
        let mut m = MissionGraph::new(r[0]);
        m.add_edge(MissionEdge::open(r[0], r[1])); // start → vault
        m.add_edge(MissionEdge::open(r[1], r[2]).one_way()); // vault →(drop)→ depths
        m.add_edge(MissionEdge::gated(r[2], r[3], Rule::has(cap("key")))); // depths → goal
        m.set_goal(r[3]);
        // The key is back in the vault, before the drop.
        m.add_location(
            LocationId(0),
            Location {
                scope: r[1],
                slot: 0,
            },
        );

        let placements: BTreeMap<LocationId, ObjectId> =
            [(LocationId(0), item("key"))].into_iter().collect();
        let grants: BTreeMap<ObjectId, ObjectId> =
            [(item("key"), cap("key"))].into_iter().collect();
        (g, m, placements, grants)
    }

    #[test]
    fn a_solvable_world_can_still_be_soft_locked() {
        // The whole reason this milestone exists: M09's guarantee passes, and the world is a trap.
        let (_, mission, placements, grants) = trap_world();

        // Solvable — the key is obtainable and the goal is reachable.
        let sweep = mission.sweep(&BTreeSet::new(), &placements, &grants);
        assert!(sweep.reaches(mission.goal().unwrap()), "M09's bar is met");
        assert!(sweep.held.contains(&cap("key")));

        // And yet: drop without the key and you are finished.
        let analysis = SoftlockAnalyzer::new(&mission, &placements, &grants).analyze();
        assert!(
            !analysis.is_un_softlockable(),
            "the stronger property must fail here"
        );
        assert_eq!(analysis.hazards.len(), 1);
        let hazard = &analysis.hazards[0];
        assert!(
            hazard.holding.is_empty(),
            "the trap is crossing with nothing"
        );
        assert_eq!(hazard.kind, SoftlockKind::GoalUnreachable);
    }

    #[test]
    fn taking_the_key_first_is_correctly_not_a_hazard() {
        // The analysis must not flag the safe route, or it would report every one-way edge.
        let (_, mission, placements, grants) = trap_world();
        let analysis = SoftlockAnalyzer::new(&mission, &placements, &grants).analyze();
        assert!(
            !analysis
                .hazards
                .iter()
                .any(|h| h.holding.contains(&cap("key"))),
            "crossing *with* the key is fine and must not be reported"
        );
    }

    #[test]
    fn moving_the_key_past_the_drop_makes_the_world_safe() {
        // The same topology, one placement different — and the trap is gone.
        let (_, r) = rooms(4);
        let mut m = MissionGraph::new(r[0]);
        m.add_edge(MissionEdge::open(r[0], r[1]));
        m.add_edge(MissionEdge::open(r[1], r[2]).one_way());
        m.add_edge(MissionEdge::gated(r[2], r[3], Rule::has(cap("key"))));
        m.set_goal(r[3]);
        m.add_location(
            LocationId(0),
            Location {
                scope: r[2],
                slot: 0,
            },
        ); // key beyond the drop

        let placements: BTreeMap<LocationId, ObjectId> =
            [(LocationId(0), item("key"))].into_iter().collect();
        let grants: BTreeMap<ObjectId, ObjectId> =
            [(item("key"), cap("key"))].into_iter().collect();

        let analysis = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
        assert!(analysis.is_un_softlockable(), "{:?}", analysis.hazards);
    }

    #[test]
    fn a_recovery_point_rescues_an_otherwise_trapping_drop() {
        let (_, mut mission, placements, grants) = trap_world();
        assert!(!SoftlockAnalyzer::new(&mission, &placements, &grants)
            .analyze()
            .is_un_softlockable());

        // A warp back out of the depths — how real games solve exactly this.
        let landing = mission.edges()[1].to;
        mission.add_recovery(landing);

        let analysis = SoftlockAnalyzer::new(&mission, &placements, &grants).analyze();
        assert!(
            analysis.is_un_softlockable(),
            "reaching a recovery point un-strands the player"
        );
    }

    #[test]
    fn repair_removes_every_hazard() {
        let (_, mut mission, placements, grants) = trap_world();
        let before = SoftlockAnalyzer::new(&mission, &placements, &grants).analyze();
        assert!(!before.is_un_softlockable());

        let repairs = before.repair(&mut mission);
        assert_eq!(repairs, vec![Repair::MadeReversible { edge: 1 }]);

        let after = SoftlockAnalyzer::new(&mission, &placements, &grants).analyze();
        assert!(
            after.is_un_softlockable(),
            "the repair must actually fix it"
        );
        assert_eq!(
            after.commits_checked, 0,
            "there are no one-way commits left"
        );
    }

    #[test]
    fn a_world_with_no_one_way_edges_is_trivially_safe() {
        // Monotone capabilities mean collecting can never strand you, so without commits there is
        // nothing to check — and the pass should cost nothing rather than enumerate anyway.
        let (_, r) = rooms(4);
        let mut m = MissionGraph::new(r[0]);
        for w in r.windows(2) {
            m.add_edge(MissionEdge::open(w[0], w[1]));
        }
        m.set_goal(r[3]);
        let analysis = SoftlockAnalyzer::new(&m, &BTreeMap::new(), &BTreeMap::new()).analyze();
        assert!(analysis.is_un_softlockable());
        assert_eq!(analysis.commits_checked, 0);
        assert_eq!(analysis.states_examined, 0);
    }

    #[test]
    fn an_incomplete_analysis_is_not_reported_as_safe() {
        // The dangerous failure mode: an unanswered question read as a "no".
        let (_, r) = rooms(3);
        let mut m = MissionGraph::new(r[0]);
        m.add_edge(MissionEdge::open(r[0], r[1]).one_way());
        m.set_goal(r[2]);

        // No goal declared → undefined, not safe.
        let mut no_goal = MissionGraph::new(r[0]);
        no_goal.add_edge(MissionEdge::open(r[0], r[1]).one_way());
        let a = SoftlockAnalyzer::new(&no_goal, &BTreeMap::new(), &BTreeMap::new()).analyze();
        assert_eq!(a.limit, Some(AnalysisLimit::NoGoal));
        assert!(
            !a.is_un_softlockable(),
            "no goal means undefined, never safe"
        );

        // Too many capabilities → declined, not safe.
        let mut placements = BTreeMap::new();
        let mut grants = BTreeMap::new();
        for i in 0..5 {
            m.add_location(
                LocationId(i),
                Location {
                    scope: r[1],
                    slot: i,
                },
            );
            placements.insert(LocationId(i), item(&format!("i{i}")));
            grants.insert(item(&format!("i{i}")), cap(&format!("c{i}")));
        }
        let limited = SoftlockAnalyzer::new(&m, &placements, &grants).with_max_capabilities(2);
        let a = limited.analyze();
        assert!(matches!(
            a.limit,
            Some(AnalysisLimit::TooManyCapabilities { .. })
        ));
        assert!(!a.is_un_softlockable());
        assert!(a
            .limit
            .unwrap()
            .to_string()
            .contains("exceeds the analysis bound"));
    }

    #[test]
    fn safety_pruning_keeps_the_cost_down() {
        // With several capabilities and a safe drop, monotone pruning should stop the pass exploring
        // the whole lattice — the property that makes the analysis affordable.
        let (_, r) = rooms(5);
        let mut m = MissionGraph::new(r[0]);
        m.add_edge(MissionEdge::open(r[0], r[1]));
        m.add_edge(MissionEdge::open(r[1], r[2]).one_way());
        m.add_edge(MissionEdge::open(r[2], r[3]));
        m.add_edge(MissionEdge::open(r[3], r[4]));
        m.set_goal(r[4]);

        let mut placements = BTreeMap::new();
        let mut grants = BTreeMap::new();
        for i in 0..6u32 {
            m.add_location(
                LocationId(i),
                Location {
                    scope: r[0],
                    slot: i,
                },
            );
            placements.insert(LocationId(i), item(&format!("i{i}")));
            grants.insert(item(&format!("i{i}")), cap(&format!("c{i}")));
        }

        let analysis = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
        assert!(analysis.is_un_softlockable());
        // 2^6 = 64 sets exist; the empty set is safe, so everything above it is pruned.
        assert!(
            analysis.states_examined <= 2,
            "pruning should collapse the lattice, examined {}",
            analysis.states_examined
        );
    }

    #[test]
    fn hazards_carry_a_reproduction_case() {
        let (_, mission, placements, grants) = trap_world();
        let analysis = SoftlockAnalyzer::new(&mission, &placements, &grants).analyze();
        let hazard = &analysis.hazards[0];
        assert_eq!(hazard.edge, 1);
        assert_eq!(hazard.from, mission.edges()[1].from);
        assert_eq!(hazard.to, mission.edges()[1].to);
        // The message names what a dev needs to reproduce it.
        assert!(hazard.to_string().contains("one-way edge 1"));
    }

    #[test]
    fn analysis_is_deterministic() {
        let (_, mission, placements, grants) = trap_world();
        let a = SoftlockAnalyzer::new(&mission, &placements, &grants).analyze();
        let b = SoftlockAnalyzer::new(&mission, &placements, &grants).analyze();
        assert_eq!(a, b);
    }

    #[test]
    fn combinations_are_complete_and_ordered() {
        let items = [1u32, 2, 3, 4];
        assert_eq!(combinations(&items, 0).len(), 1);
        assert_eq!(combinations(&items, 1).len(), 4);
        assert_eq!(combinations(&items, 2).len(), 6);
        assert_eq!(combinations(&items, 4).len(), 1);
        assert_eq!(combinations(&items, 5).len(), 0);
        assert_eq!(combinations(&items, 2)[0], vec![1, 2]);
        // Deterministic across calls.
        assert_eq!(combinations(&items, 3), combinations(&items, 3));
    }
}
