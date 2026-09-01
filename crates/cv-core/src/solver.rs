//! **L2's algorithm** — placing progression items so the world is solvable *by construction*, and
//! shaping how linear that world feels.
//!
//! # Solvable by construction, not by checking
//!
//! The naive approach is to place items randomly and then test whether the result can be completed,
//! retrying on failure. That degrades badly: as gating deepens, the odds of a random arrangement being
//! solvable collapse, and the generator spends its time rejecting worlds.
//!
//! **Assumed fill** inverts it. Items are placed one at a time, and each is placed only somewhere
//! accessible *without it*. That single rule makes a circular dependency — the key behind the door it
//! opens — impossible to create, so there is nothing to check afterwards. Every world the solver
//! returns is completable, and the sphere analysis it produces along the way *proves* it.
//!
//! # How much the world loops
//!
//! `cycle_density` — `0.0` is a pure chain; `1.0` adds every shortcut it reasonably can, producing
//! fork-and-reconverge structure. It is an input rather than an artefact because a generator whose
//! looping was an accident of implementation would give a developer nothing to choose.
//!
//! ▶ It becomes a **user-authored dial** when the dial machinery lands. That is the right home for it
//! and the design says why: *"a dial exists for what only the generator can decide"* — no Actor can
//! state *"this world has many loops"*.
//!
//! ⚠ **Key-to-lock distance is deliberately not a second dial here.** A pre-v0.2 `progression_locality`
//! dial used to sit beside this one and was deleted, because *"a constraint exists for what content can
//! state, and key-to-lock distance is stateable"* — the door writes `MinDistanceFrom` itself, and a
//! dial would be a second way to say the same thing.

use crate::dial::{DialId, ResolvedDials};
use crate::mission::{Accessibility, Location, LocationId, MissionEdge, MissionGraph, Rule};
use crate::node::{Node, NodeGraph, NodeKind};
use crate::object::ObjectId;
use crate::unlock::GrantMap;
use crate::Handle;
use cv_determinism::Rng;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

/// Why a world could not be made solvable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolveError {
    /// More progression items than places to put them.
    NotEnoughLocations { items: usize, locations: usize },
    /// An item had nowhere accessible to go — the world is gated in a way that admits no solution.
    NoAccessibleLocation {
        item: ObjectId,
        placed_so_far: usize,
    },
    /// The world was built, but something required by L1 could not be placed.
    UnmetDemand { content: ObjectId },
}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolveError::NotEnoughLocations { items, locations } => write!(
                f,
                "{items} progression items but only {locations} locations — \
                 either schedule more slots or gate less"
            ),
            SolveError::NoAccessibleLocation {
                item,
                placed_so_far,
            } => write!(
                f,
                "no accessible location for {item} after placing {placed_so_far}; \
                 the gating admits no solvable arrangement"
            ),
            SolveError::UnmetDemand { content } => {
                write!(f, "{content} is required to exist but could not be placed")
            }
        }
    }
}

impl std::error::Error for SolveError {}

// ---------------------------------------------------------------------------------------------
// Solution
// ---------------------------------------------------------------------------------------------

/// Why an item ended up where it did.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacementTrace {
    /// The item.
    pub item: ObjectId,
    /// Where it went.
    pub location: LocationId,
    /// How many locations were legal at the time.
    pub candidates: usize,
    /// Hops from the placement to the nearest lock it opens; `None` if it gates nothing.
    pub distance_to_lock: Option<u32>,
}

/// A solved world.
#[derive(Clone, Debug, PartialEq)]
pub struct Solution {
    /// What went where.
    pub placements: BTreeMap<LocationId, ObjectId>,
    /// The progression, sphere by sphere.
    pub accessibility: Accessibility,
    /// Why each item landed where it did.
    pub traces: Vec<PlacementTrace>,
    /// How many orderings were tried before one worked.
    ///
    /// `1` is the normal case. A consistently higher number is a signal worth surfacing to a dev:
    /// the world is gated tightly enough that most arrangements fail, which usually means too few
    /// early locations rather than a bug.
    pub attempts: u32,
}

impl Solution {
    /// How many rounds of progression the world has.
    pub fn depth(&self) -> u32 {
        self.accessibility.depth()
    }

    /// Is every location accessible?
    ///
    /// `false` is not necessarily wrong — a world may hold optional pockets — but it is worth knowing.
    pub fn fully_accessible(&self, mission: &MissionGraph) -> bool {
        mission.location_count() == self.accessibility.locations.len()
    }
}

// ---------------------------------------------------------------------------------------------
// The solver
// ---------------------------------------------------------------------------------------------

/// Places progression items so the result is solvable by construction.
pub struct Solver<'a> {
    graph: &'a NodeGraph,
    /// How often a shortcut closes a loop, 0..1 — **the fallback** when no dial is authored.
    cycle_density: f64,
    /// The resolved dial table, and which dial in it supplies `cycle_density`.
    ///
    /// ⚠ **A dial that reaches nothing is not a dial.** `cycle_density` is the design's own example of
    /// a legitimate one — *"no Actor can say 'this world has many loops'"* — so if an authored dial
    /// could not reach the solver, the whole category would have no members.
    dials: Option<(&'a ResolvedDials, DialId)>,
    /// Item content id → the unlocks obtaining it grants.
    grants: GrantMap,
    /// Unlocks the player starts with.
    initial: BTreeSet<ObjectId>,
}

impl<'a> Solver<'a> {
    /// A solver over a scope graph and dial set.
    pub fn new(graph: &'a NodeGraph) -> Self {
        Solver {
            graph,
            cycle_density: 0.35,
            dials: None,
            grants: BTreeMap::new(),
            initial: BTreeSet::new(),
        }
    }

    /// How often a shortcut closes a loop, 0..1.
    ///
    /// ⚠ **The fallback for a project that authors no such dial**, not the only way to set it. The
    /// core ships no dials, so a project with none still needs a number — but the moment one is
    /// authored, [`Self::reading_dial`] takes over and this stops being consulted.
    pub fn with_cycle_density(mut self, density: f64) -> Self {
        self.cycle_density = density.clamp(0.0, 1.0);
        self
    }

    /// **Read `cycle_density` from an authored dial**, per scope.
    ///
    /// ⚠ **This is the read path a dial's whole existence rests on.** Without it `cycle_density` is a
    /// constructor argument and the dial channel resolves values nothing consumes.
    pub fn reading_dial(mut self, dials: &'a ResolvedDials, id: DialId) -> Self {
        self.dials = Some((dials, id));
        self
    }

    /// The density in force **where a shortcut between these two Spaces would sit**.
    ///
    /// ⚠ **At their common ancestor, not at either end.** A shortcut belongs to the scope that
    /// contains both of them, so a dial set on one Area loops *that* Area — which is the whole reason
    /// resolution walks the ladder. Reading it at one endpoint would make the answer depend on which
    /// Space the enumeration happened to visit first.
    fn density_between(&self, a: Handle<Node>, b: Handle<Node>) -> f64 {
        let Some((dials, id)) = &self.dials else {
            return self.cycle_density;
        };
        let scope = self.common_ancestor(a, b).unwrap_or(self.graph.root());
        dials
            .number(id, scope)
            .map(|d| d.clamp(0.0, 1.0))
            .unwrap_or(self.cycle_density)
    }

    /// The innermost scope containing both.
    fn common_ancestor(&self, a: Handle<Node>, b: Handle<Node>) -> Option<Handle<Node>> {
        let chain = |mut h: Handle<Node>| -> Vec<Handle<Node>> {
            let mut out = vec![h];
            while let Some(p) = self.graph.get(h).and_then(Node::parent) {
                out.push(p);
                h = p;
            }
            out
        };
        let up_a = chain(a);
        let up_b = chain(b);
        up_a.into_iter().find(|h| up_b.contains(h))
    }

    /// Declare that obtaining `item` grants `unlock`.
    ///
    /// ⚠ **Additive.** Calling this twice for one item grants both — one pickup, several separable
    /// gates. Overwriting instead would silently drop a gate.
    pub fn with_grant(mut self, item: ObjectId, unlock: ObjectId) -> Self {
        self.grants.entry(item).or_default().insert(unlock);
        self
    }

    /// Declare a unlock the player starts with.
    pub fn with_initial(mut self, unlock: ObjectId) -> Self {
        self.initial.insert(unlock);
        self
    }

    /// The item→unlocks map.
    pub fn grants(&self) -> &GrantMap {
        &self.grants
    }

    /// **Add loops to the topology**, per `cycle_density`.
    ///
    /// Considers every pair of Spaces two hops apart and closes the triangle for a fraction of them.
    /// Two hops is the sweet spot: closing a one-hop pair is a no-op, and closing a distant pair
    /// produces a corridor to nowhere rather than a loop a player would recognise.
    ///
    /// Candidate pairs are enumerated in deterministic order, and each is decided by a stream forked
    /// on the *pair's identity*, so raising the dial adds shortcuts rather than reshuffling them.
    pub fn add_cycles(&self, mission: &mut MissionGraph, rng: &Rng) -> usize {
        let spaces: Vec<Handle<Node>> = self
            .graph
            .of_kind(NodeKind::Space)
            .map(|(h, _)| h)
            .collect();
        let mut added = 0;

        // Snapshot distances before adding anything, so one shortcut does not change which pairs are
        // considered for the next — otherwise the result would depend on evaluation order.
        let distances: BTreeMap<Handle<Node>, BTreeMap<Handle<Node>, u32>> = spaces
            .iter()
            .map(|s| (*s, mission.distances_from(*s)))
            .collect();

        let decide = rng.fork("cycles");
        for (i, a) in spaces.iter().enumerate() {
            for b in spaces.iter().skip(i + 1) {
                if mission.connects(*a, *b) {
                    continue;
                }
                if distances[a].get(b) != Some(&2) {
                    continue;
                }
                // The dial in force where the shortcut would live.
                let density = self.density_between(*a, *b);
                if density <= 0.0 {
                    continue;
                }
                let pair_key = format!("{}-{}", a.to_raw(), b.to_raw());
                if decide.fork(&pair_key).chance(density) {
                    mission.add_edge(MissionEdge::open(*a, *b).shortcut());
                    added += 1;
                }
            }
        }
        added
    }

    /// **Assumed fill.** Place every item somewhere accessible without it.
    ///
    /// Items are consumed from the back of `items`. At each step the sweep assumes the player holds
    /// everything *still unplaced*, which is what lets an item legitimately sit deep in the world: the
    /// things that open the way to it are guaranteed to be placed earlier in progression than it is.
    /// How many orderings to try before declaring a gating unfillable.
    ///
    /// Assumed fill is *correct* — it can never create a circular dependency — but it is not
    /// *complete*: a run can spend the last shallow location on a late item and leave an early item
    /// homeless. That is a property of the ordering, not of the world, so a different ordering usually
    /// succeeds. Retrying a bounded number of times is what real fill algorithms do, and the bound is
    /// what keeps a genuinely unsolvable gating from spinning forever.
    const MAX_ATTEMPTS: u32 = 16;

    pub fn fill(
        &self,
        mission: &MissionGraph,
        items: &[ObjectId],
        rng: &Rng,
    ) -> Result<Solution, SolveError> {
        if items.len() > mission.location_count() {
            return Err(SolveError::NotEnoughLocations {
                items: items.len(),
                locations: mission.location_count(),
            });
        }

        let mut last_error = None;
        for attempt in 0..Self::MAX_ATTEMPTS {
            let mut pool = items.to_vec();
            if attempt > 0 {
                // A fresh ordering, deterministically derived from the attempt number.
                rng.fork("attempt")
                    .fork_index(attempt as u64)
                    .shuffle(&mut pool);
            }
            match self.try_fill(mission, &pool, rng, attempt) {
                Ok(solution) => return Ok(solution),
                Err(e) => last_error = Some(e),
            }
        }
        Err(last_error.expect("at least one attempt ran"))
    }

    /// One assumed-fill pass over a specific ordering.
    fn try_fill(
        &self,
        mission: &MissionGraph,
        items: &[ObjectId],
        rng: &Rng,
        attempt: u32,
    ) -> Result<Solution, SolveError> {
        let mut placements: BTreeMap<LocationId, ObjectId> = BTreeMap::new();
        let mut traces: Vec<PlacementTrace> = Vec::new();
        let mut pool: Vec<ObjectId> = items.to_vec();
        // Fork on the attempt too, so a retry explores different placements rather than repeating the
        // same choices against a merely-reordered pool.
        let choose = rng.fork("fill").fork_index(attempt as u64);

        while let Some(item) = pool.pop() {
            // Assume everything still unplaced is held. Placed items are *not* assumed — they are
            // found by sweeping, which is what keeps the dependency order honest.
            let mut assumed = self.initial.clone();
            for remaining in &pool {
                if let Some(unlocks) = self.grants.get(remaining) {
                    assumed.extend(unlocks.iter().copied());
                }
            }

            let accessible = mission.sweep(&assumed, &placements, &self.grants);
            let open: Vec<LocationId> = accessible
                .locations
                .iter()
                .filter(|l| !placements.contains_key(l))
                .copied()
                .collect();

            if open.is_empty() {
                return Err(SolveError::NoAccessibleLocation {
                    item,
                    placed_so_far: placements.len(),
                });
            }

            let (location, distance) =
                self.choose_location(mission, item, &open, &choose.fork(&item.to_string()));
            traces.push(PlacementTrace {
                item,
                location,
                candidates: open.len(),
                distance_to_lock: distance,
            });
            placements.insert(location, item);
        }

        let accessibility = mission.sweep(&self.initial, &placements, &self.grants);
        // Reverse so the trace reads in placement order rather than pop order.
        traces.reverse();
        Ok(Solution {
            placements,
            accessibility,
            traces,
            attempts: attempt + 1,
        })
    }

    /// Pick where an item goes, uniformly among the legal locations.
    ///
    /// ▶ **M07 P07 restores key-to-lock distance control**, driven by the door's own
    /// `MinDistanceFrom` / `MaxDistanceFrom` constraints. ⚠ It is **not** restored as a dial: the design
    /// refuses one, because *"a dial exists for what only the generator can decide; a constraint for what
    /// content can state, and key-to-lock distance is stateable"*
    /// ([`05-object-model.md`](../../../.notes/Design/v0.2b/05-object-model.md) §4.2). The pre-v0.1
    /// `progression_locality` dial that used to weight this was deleted at M04a.
    fn choose_location(
        &self,
        mission: &MissionGraph,
        item: ObjectId,
        open: &[LocationId],
        rng: &Rng,
    ) -> (LocationId, Option<u32>) {
        // The locks this item opens, if any.
        // ⚠ Every unlock this item grants, not just one: an item that opens two lock families is
        // placed against both, and taking only the first would ignore half its own consequences.
        let lock_scopes: Vec<Handle<Node>> = match self.grants.get(&item) {
            Some(unlocks) => unlocks
                .iter()
                .flat_map(|u| mission.locks_for(*u).map(|e| e.from))
                .collect(),
            None => Vec::new(),
        };

        let mut picker = rng.fork("where");

        if lock_scopes.is_empty() {
            // Nothing to be near; uniform.
            let idx = picker.below(open.len() as u64) as usize;
            return (open[idx], None);
        }

        // Distance from each open location to the nearest lock it opens. Kept because the trace
        // reports it and a reader wants it — not used to bias the choice.
        let distance_maps: Vec<BTreeMap<Handle<Node>, u32>> = lock_scopes
            .iter()
            .map(|s| mission.distances_from(*s))
            .collect();
        let scored: Vec<(LocationId, u32)> = open
            .iter()
            .map(|loc| {
                let scope = mission
                    .locations()
                    .find(|(id, _)| id == loc)
                    .map(|(_, l): (LocationId, Location)| l.scope);
                let d = scope
                    .map(|s| {
                        distance_maps
                            .iter()
                            .filter_map(|m| m.get(&s).copied())
                            .min()
                            .unwrap_or(u32::MAX)
                    })
                    .unwrap_or(u32::MAX);
                (*loc, d)
            })
            .collect();

        let idx = picker.below(scored.len() as u64) as usize;
        (scored[idx].0, Some(scored[idx].1))
    }

    /// Turn a fraction of edges into **one-way commits** — a drop you cannot climb back up.
    ///
    /// These are what make a world feel like it has consequences, and they are also the only way a
    /// player can strand themselves (unlocks are monotone, so collecting never hurts). The
    /// un-softlockable pass exists to check exactly what this introduces, and should be run
    /// afterwards — generating commits without validating them is how a shipped softlock happens.
    ///
    /// Shortcuts are left alone: a loop-closing edge that only worked one way would defeat its purpose.
    pub fn add_one_way_commits(
        &self,
        mission: &mut MissionGraph,
        fraction: f64,
        rng: &Rng,
    ) -> usize {
        let distances = mission.distances_from(mission.start());
        let decide = rng.fork("one_way");

        // Only edges leading *away* from the start, so a commit always sits between the player and
        // something new rather than sealing the route behind them at the door.
        let candidates: Vec<usize> = mission
            .edges()
            .iter()
            .enumerate()
            .filter(|(_, e)| e.reversible && !e.is_shortcut)
            .filter(
                |(_, e)| match (distances.get(&e.from), distances.get(&e.to)) {
                    (Some(a), Some(b)) => a != b,
                    _ => false,
                },
            )
            .map(|(i, _)| i)
            .collect();

        let mut made = 0;
        for index in candidates {
            if decide.fork(&index.to_string()).chance(fraction) {
                mission.make_one_way(index);
                made += 1;
            }
        }
        made
    }

    /// Gate edges so the world has progression at all.
    ///
    /// Walks the graph outward from the start and gates a fraction of the edges that lead *away* from
    /// it, so a gate always sits between the player and something new rather than sealing a dead end.
    /// Which unlock gates which edge is drawn deterministically from `unlocks`.
    pub fn gate_edges(
        &self,
        mission: &mut MissionGraph,
        unlocks: &[ObjectId],
        gate_fraction: f64,
        rng: &Rng,
    ) -> usize {
        if unlocks.is_empty() {
            return 0;
        }
        let distances = mission.distances_from(mission.start());
        let decide = rng.fork("gates");
        let mut gated = 0;

        // Collect first: gating changes nothing about distances, but taking a snapshot keeps the
        // decision independent of iteration order.
        let candidates: Vec<(usize, u32)> = mission
            .edges()
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_gated() && !e.is_shortcut)
            .filter_map(|(i, e)| {
                let (a, b) = (distances.get(&e.from)?, distances.get(&e.to)?);
                // Only edges that lead further from the start.
                (a != b).then_some((i, *a.max(b)))
            })
            .collect();

        // Map depth onto the unlock list *proportionally*, not by raw index.
        //
        // Using the depth directly is wrong and produces unsolvable worlds: the first edge out of the
        // start would be gated on unlock #1, whose item can only live beyond that very edge. The
        // world seals itself at the door. Scaling by the world's actual depth keeps shallow gates on
        // early unlocks and deep gates on late ones, which is what "gating follows progression"
        // has to mean.
        let max_depth = candidates.iter().map(|(_, d)| *d).max().unwrap_or(1).max(1) as usize;
        for (index, depth) in candidates {
            if !decide.fork(&index.to_string()).chance(gate_fraction) {
                continue;
            }
            let scaled = depth.saturating_sub(1) as usize * unlocks.len() / max_depth;
            let pick = scaled.min(unlocks.len() - 1);
            mission.gate_edge(index, Rule::has(unlocks[pick]));
            gated += 1;
        }
        gated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::Location;

    fn cap(name: &str) -> ObjectId {
        ObjectId::derived("unlock", name)
    }

    fn item(name: &str) -> ObjectId {
        ObjectId::derived("item", name)
    }

    /// A grid-ish world of `n` rooms in a chain.
    fn chain_world(n: usize) -> (NodeGraph, Vec<Handle<Node>>) {
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "reach").unwrap();
        let area = g.add_child(reach, "area").unwrap();
        let rooms: Vec<Handle<Node>> = (0..n)
            .map(|i| g.add_child(area, format!("room_{i}")).unwrap())
            .collect();
        for w in rooms.windows(2) {
            g.connect(w[0], w[1]).unwrap();
        }
        (g, rooms)
    }

    fn mission_with_locations(g: &NodeGraph, rooms: &[Handle<Node>]) -> MissionGraph {
        let mut m = MissionGraph::from_scopes(g, rooms[0]);
        for (i, room) in rooms.iter().enumerate() {
            m.add_location(
                LocationId(i as u32),
                Location {
                    scope: *room,
                    slot: 0,
                },
            );
        }
        m
    }

    // ⚠ `dials_resolve_nearest_override_first` was deleted at M04a. It tested outward-in override
    // resolution for `progression_locality`, a dial the design refuses. **Dial inheritance itself is
    // design-backed** — outward-in, inner scope wins — and returns at M09 P04a over real dials.

    #[test]
    fn assumed_fill_never_puts_a_key_behind_its_own_door() {
        let (g, rooms) = chain_world(6);
        let mut mission = mission_with_locations(&g, &rooms);
        let dash = cap("dash");
        let key = item("key");

        // Gate the middle of the chain on the dash.
        mission.gate_edge(2, Rule::has(dash));

        let solver = Solver::new(&g).with_grant(key, dash);
        let solution = solver
            .fill(&mission, &[key], &Rng::new(1))
            .expect("solvable");

        // The key must be in a room accessible *before* the gate.
        let loc = solution
            .placements
            .iter()
            .find(|(_, v)| **v == key)
            .map(|(k, _)| *k)
            .unwrap();
        let scope = mission
            .locations()
            .find(|(id, _)| *id == loc)
            .unwrap()
            .1
            .scope;
        let before_gate = [rooms[0], rooms[1], rooms[2]];
        assert!(
            before_gate.contains(&scope),
            "the key must be findable without itself"
        );
        assert!(
            solution.accessibility.accessible(*rooms.last().unwrap()),
            "and the world completes"
        );
    }

    #[test]
    fn every_solved_world_is_completable() {
        // The property the algorithm exists to guarantee, across many seeds and gate arrangements.
        let (g, rooms) = chain_world(8);
        let caps: Vec<ObjectId> = (0..3).map(|i| cap(&format!("c{i}"))).collect();
        let items: Vec<ObjectId> = (0..3).map(|i| item(&format!("i{i}"))).collect();

        for seed in 0..40u64 {
            let mut mission = mission_with_locations(&g, &rooms);
            let mut solver = Solver::new(&g);
            for (it, c) in items.iter().zip(&caps) {
                solver = solver.with_grant(*it, *c);
            }
            let rng = Rng::new(seed);
            solver.gate_edges(&mut mission, &caps, 0.6, &rng);

            let solution = solver
                .fill(&mission, &items, &rng)
                .expect("must be solvable");
            // Every item is obtainable...
            for c in &caps {
                assert!(
                    solution.accessibility.held.contains(c),
                    "seed {seed}: {c} unobtainable"
                );
            }
            // ...and the far end of the world is reached.
            assert!(
                solution.accessibility.accessible(*rooms.last().unwrap()),
                "seed {seed}: the world does not complete"
            );
        }
    }

    // ⚠ `locality_zero_keeps_keys_near_their_locks` was deleted at M04a: it measured
    // `progression_locality`, which the design refuses. ▶ **M07 P07 restores the behaviour** driven by
    // the door's own `MinDistanceFrom` / `MaxDistanceFrom`, and this test returns against those.

    #[test]
    fn cycle_density_controls_how_much_the_world_loops() {
        let (g, rooms) = chain_world(10);

        let count_shortcuts = |density: f64| {
            let solver = Solver::new(&g).with_cycle_density(density);
            let mut mission = mission_with_locations(&g, &rooms);
            solver.add_cycles(&mut mission, &Rng::new(7));
            mission.shortcut_count()
        };

        assert_eq!(count_shortcuts(0.0), 0, "a chain stays a chain");
        let mid = count_shortcuts(0.5);
        let dense = count_shortcuts(1.0);
        assert!(mid > 0, "some loops appear");
        assert!(
            dense >= mid,
            "raising the dial does not remove loops ({dense} vs {mid})"
        );
    }

    #[test]
    fn solving_is_deterministic() {
        let (g, rooms) = chain_world(7);
        let caps: Vec<ObjectId> = (0..2).map(|i| cap(&format!("c{i}"))).collect();
        let items: Vec<ObjectId> = (0..2).map(|i| item(&format!("i{i}"))).collect();

        let solve = || {
            let mut mission = mission_with_locations(&g, &rooms);
            let mut solver = Solver::new(&g);
            for (it, c) in items.iter().zip(&caps) {
                solver = solver.with_grant(*it, *c);
            }
            let rng = Rng::new(0xD00D);
            solver.add_cycles(&mut mission, &rng);
            solver.gate_edges(&mut mission, &caps, 0.5, &rng);
            (
                mission.clone(),
                solver.fill(&mission, &items, &rng).unwrap(),
            )
        };
        let (m1, s1) = solve();
        let (m2, s2) = solve();
        assert_eq!(m1, m2);
        assert_eq!(s1, s2);
    }

    #[test]
    fn too_many_items_is_reported_not_silently_dropped() {
        let (g, rooms) = chain_world(3);
        let mut mission = MissionGraph::from_scopes(&g, rooms[0]);
        mission.add_location(
            LocationId(0),
            Location {
                scope: rooms[0],
                slot: 0,
            },
        );

        let solver = Solver::new(&g);
        let items = [item("a"), item("b")];
        assert_eq!(
            solver.fill(&mission, &items, &Rng::new(1)),
            Err(SolveError::NotEnoughLocations {
                items: 2,
                locations: 1
            })
        );
    }

    #[test]
    fn an_unsatisfiable_gating_fails_loudly() {
        // A gate that nothing can open: the far room is unreachable, so there is nowhere legal to put
        // the item that lives beyond it.
        let (g, rooms) = chain_world(3);
        let mut mission = MissionGraph::from_scopes(&g, rooms[0]);
        // Only one location, and it is behind a permanently sealed edge.
        mission.gate_edge(0, Rule::Never);
        mission.add_location(
            LocationId(0),
            Location {
                scope: rooms[2],
                slot: 0,
            },
        );

        let solver = Solver::new(&g);
        let result = solver.fill(&mission, &[item("a")], &Rng::new(1));
        assert!(matches!(
            result,
            Err(SolveError::NoAccessibleLocation { .. })
        ));
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no accessible location"));
    }

    #[test]
    fn the_solution_explains_each_placement() {
        let (g, rooms) = chain_world(6);
        let mut mission = mission_with_locations(&g, &rooms);
        let dash = cap("dash");
        let key = item("key");
        mission.gate_edge(3, Rule::has(dash));

        let solver = Solver::new(&g).with_grant(key, dash);
        let solution = solver.fill(&mission, &[key], &Rng::new(3)).unwrap();

        assert_eq!(solution.traces.len(), 1);
        let t = &solution.traces[0];
        assert_eq!(t.item, key);
        assert!(t.candidates > 0);
        assert!(
            t.distance_to_lock.is_some(),
            "this key gates something, so distance is meaningful"
        );
    }

    #[test]
    fn items_that_gate_nothing_are_placed_freely() {
        let (g, rooms) = chain_world(5);
        let mission = mission_with_locations(&g, &rooms);
        // No grants at all — pure treasure.
        let solver = Solver::new(&g);
        let items = [item("gold"), item("gem")];
        let solution = solver.fill(&mission, &items, &Rng::new(9)).unwrap();
        assert_eq!(solution.placements.len(), 2);
        for t in &solution.traces {
            assert_eq!(t.distance_to_lock, None, "nothing to be near");
        }
    }
}
