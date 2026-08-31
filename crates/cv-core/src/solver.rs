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
//! # The two dials, and why they are inputs rather than emergent
//!
//! How linear a world feels is a design decision, not an artefact of the algorithm:
//!
//! * **`progression_locality`** — how far a key may sit from the lock it opens. `0.0` puts it in or
//!   beside the gated room (Portal-style, no backtracking); `1.0` allows anywhere accessible
//!   (MP1-style, heavy backtracking).
//! * **`cycle_density`** — how much the topology loops. `0.0` is a pure chain; `1.0` adds every
//!   shortcut it reasonably can, producing fork-and-reconverge structure.
//!
//! If these were left implicit, a generator's linearity would be an accident of implementation and a
//! dev could not choose it. They resolve **content override → nearest enclosing scope → world
//! default**, so a mostly non-linear world can contain a strictly linear stretch without special
//! cases. See `01-core/pipeline.md` for the full linearity model.

use crate::mission::{Accessibility, Location, LocationId, MissionEdge, MissionGraph, Rule};
use crate::node::{Node, NodeGraph, NodeKind};
use crate::object::ObjectId;
use crate::unlock::GrantMap;
use crate::Handle;
use cv_determinism::{math, Rng};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// ---------------------------------------------------------------------------------------------
// Linearity dials
// ---------------------------------------------------------------------------------------------

/// The dials controlling how non-linear a world is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Linearity {
    /// How far a key may sit from its lock. `0.0` adjacent, `1.0` anywhere.
    pub progression_locality: f64,
    /// How much the topology loops. `0.0` a chain, `1.0` densely connected.
    pub cycle_density: f64,
}

impl Linearity {
    /// Portal-style: keys beside their locks, no loops.
    pub const LINEAR: Linearity = Linearity {
        progression_locality: 0.0,
        cycle_density: 0.0,
    };

    /// MP1-style: keys anywhere, dense loops and shortcuts.
    pub const OPEN: Linearity = Linearity {
        progression_locality: 1.0,
        cycle_density: 0.8,
    };

    /// Custom values, clamped to `[0, 1]`.
    pub fn new(progression_locality: f64, cycle_density: f64) -> Self {
        Linearity {
            progression_locality: math::saturate(progression_locality),
            cycle_density: math::saturate(cycle_density),
        }
    }
}

impl Default for Linearity {
    fn default() -> Self {
        // A middle default: some backtracking, some loops. Neither extreme is a safe assumption.
        Linearity {
            progression_locality: 0.5,
            cycle_density: 0.35,
        }
    }
}

/// A partial override of the dials, for a scope or a piece of content.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinearityOverride {
    /// Override the locality dial.
    pub progression_locality: Option<f64>,
    /// Override the cycle dial.
    pub cycle_density: Option<f64>,
}

impl LinearityOverride {
    /// Override only the locality dial.
    pub fn locality(v: f64) -> Self {
        LinearityOverride {
            progression_locality: Some(math::saturate(v)),
            cycle_density: None,
        }
    }

    /// Override only the cycle dial.
    pub fn cycles(v: f64) -> Self {
        LinearityOverride {
            progression_locality: None,
            cycle_density: Some(math::saturate(v)),
        }
    }

    /// Apply this override onto a base.
    pub fn apply_to(self, base: Linearity) -> Linearity {
        Linearity {
            progression_locality: self
                .progression_locality
                .unwrap_or(base.progression_locality),
            cycle_density: self.cycle_density.unwrap_or(base.cycle_density),
        }
    }
}

/// Resolves the dials for a given scope, honouring the override chain.
///
/// The chain is what makes mixing possible: **content override → nearest enclosing scope → world
/// default**. A heavily-backtracking world can hold one strictly self-contained Area simply by
/// overriding it there; nothing else needs to know.
#[derive(Clone, Debug, Default)]
pub struct LinearityResolver {
    world: Linearity,
    per_scope: BTreeMap<Handle<Node>, LinearityOverride>,
}

impl LinearityResolver {
    /// A resolver with a world-level default.
    pub fn new(world: Linearity) -> Self {
        LinearityResolver {
            world,
            per_scope: BTreeMap::new(),
        }
    }

    /// Override the dials for a scope and everything inside it.
    pub fn override_scope(&mut self, scope: Handle<Node>, over: LinearityOverride) -> &mut Self {
        self.per_scope.insert(scope, over);
        self
    }

    /// The world-level default.
    pub fn world(&self) -> Linearity {
        self.world
    }

    /// The dials in force at a scope.
    ///
    /// Walks outward from the scope, so the **nearest** enclosing override wins — a Space override
    /// beats its Area's, which beats the world's.
    pub fn at(&self, graph: &NodeGraph, scope: Handle<Node>) -> Linearity {
        let mut chain: Vec<Handle<Node>> = vec![scope];
        chain.extend(graph.ancestors_of(scope));
        for node in chain {
            if let Some(over) = self.per_scope.get(&node) {
                return over.apply_to(self.world);
            }
        }
        self.world
    }
}

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
    /// The locality dial in force at the chosen scope.
    pub locality: f64,
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
    linearity: &'a LinearityResolver,
    /// Item content id → the unlocks obtaining it grants.
    grants: GrantMap,
    /// Unlocks the player starts with.
    initial: BTreeSet<ObjectId>,
}

impl<'a> Solver<'a> {
    /// A solver over a scope graph and dial set.
    pub fn new(graph: &'a NodeGraph, linearity: &'a LinearityResolver) -> Self {
        Solver {
            graph,
            linearity,
            grants: BTreeMap::new(),
            initial: BTreeSet::new(),
        }
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
                let density = self.linearity.at(self.graph, *a).cycle_density;
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

            let (location, distance, locality) =
                self.choose_location(mission, item, &open, &choose.fork(&item.to_string()));
            traces.push(PlacementTrace {
                item,
                location,
                candidates: open.len(),
                distance_to_lock: distance,
                locality,
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

    /// Pick where an item goes, honouring the locality dial.
    ///
    /// Locality is applied as a **bias, not a filter**. Filtering would make a low dial unsatisfiable
    /// whenever no nearby location happened to be open, turning a preference into a failure; biasing
    /// degrades to "the closest available" instead. `1.0` is uniform over everything accessible.
    fn choose_location(
        &self,
        mission: &MissionGraph,
        item: ObjectId,
        open: &[LocationId],
        rng: &Rng,
    ) -> (LocationId, Option<u32>, f64) {
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

        // Locality is read at the *lock*, since that is the part of the world whose character the dev
        // was describing when they set it.
        let locality = match lock_scopes.first() {
            Some(scope) => self.linearity.at(self.graph, *scope).progression_locality,
            None => self.linearity.world().progression_locality,
        };

        let mut picker = rng.fork("where");

        if lock_scopes.is_empty() {
            // Nothing to be near; uniform.
            let idx = picker.below(open.len() as u64) as usize;
            return (open[idx], None, locality);
        }

        // Distance from each open location to the nearest lock.
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

        let max_d = scored
            .iter()
            .map(|(_, d)| *d)
            .filter(|d| *d != u32::MAX)
            .max()
            .unwrap_or(0);

        // Weight each candidate. At locality 0 the nearest dominates; at 1 everything is equal.
        let weights: Vec<f64> = scored
            .iter()
            .map(|(_, d)| {
                if *d == u32::MAX {
                    return 0.001; // unreachable-by-hops: possible, but a last resort
                }
                let normalised = if max_d == 0 {
                    0.0
                } else {
                    *d as f64 / max_d as f64
                };
                // closeness ∈ (0, 1]: 1 at the lock, falling off with distance.
                let closeness = 1.0 - normalised;
                // Blend between "strongly prefer close" and "no preference".
                math::lerp(closeness * closeness + 0.001, 1.0, locality)
            })
            .collect();

        let index = picker.weighted_choice(&weights);
        (scored[index].0, Some(scored[index].1), locality)
    }

    /// Turn a fraction of edges into **one-way commits** — a drop you cannot climb back up.
    ///
    /// These are what make a world feel like it has consequences, and they are also the only way a
    /// player can strand themselves (unlocks are monotone, so collecting never hurts). The
    /// un-softlockable pass (M10) exists to check exactly what this introduces, and should be run
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

    #[test]
    fn dials_resolve_nearest_override_first() {
        let (mut g, rooms) = chain_world(3);
        let area = g.scope_of(rooms[0], NodeKind::Area).unwrap();
        let _ = &mut g;

        let mut resolver = LinearityResolver::new(Linearity::OPEN);
        resolver.override_scope(area, LinearityOverride::locality(0.5));
        resolver.override_scope(rooms[1], LinearityOverride::locality(0.0));

        // The room's own override wins over its Area's.
        assert_eq!(resolver.at(&g, rooms[1]).progression_locality, 0.0);
        // A sibling with no override falls back to the Area.
        assert_eq!(resolver.at(&g, rooms[0]).progression_locality, 0.5);
        // Partial overrides leave the other dial alone.
        assert_eq!(
            resolver.at(&g, rooms[0]).cycle_density,
            Linearity::OPEN.cycle_density
        );
    }

    #[test]
    fn assumed_fill_never_puts_a_key_behind_its_own_door() {
        let (g, rooms) = chain_world(6);
        let mut mission = mission_with_locations(&g, &rooms);
        let dash = cap("dash");
        let key = item("key");

        // Gate the middle of the chain on the dash.
        mission.gate_edge(2, Rule::has(dash));

        let resolver = LinearityResolver::new(Linearity::default());
        let solver = Solver::new(&g, &resolver).with_grant(key, dash);
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
            let resolver = LinearityResolver::new(Linearity::default());
            let mut solver = Solver::new(&g, &resolver);
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

    #[test]
    fn locality_zero_keeps_keys_near_their_locks() {
        let (g, rooms) = chain_world(10);
        let dash = cap("dash");
        let key = item("key");

        let near = LinearityResolver::new(Linearity::new(0.0, 0.0));
        let far = LinearityResolver::new(Linearity::new(1.0, 0.0));

        let mut near_total = 0u32;
        let mut far_total = 0u32;
        for seed in 0..30u64 {
            for (resolver, total) in [(&near, &mut near_total), (&far, &mut far_total)] {
                let mut mission = mission_with_locations(&g, &rooms);
                mission.gate_edge(6, Rule::has(dash)); // gate deep in the chain
                let solver = Solver::new(&g, resolver).with_grant(key, dash);
                let solution = solver.fill(&mission, &[key], &Rng::new(seed)).unwrap();
                *total += solution.traces[0].distance_to_lock.unwrap_or(0);
            }
        }
        assert!(
            near_total < far_total,
            "locality 0 should place keys closer to their locks ({near_total} vs {far_total})"
        );
    }

    #[test]
    fn cycle_density_controls_how_much_the_world_loops() {
        let (g, rooms) = chain_world(10);

        let count_shortcuts = |density: f64| {
            let resolver = LinearityResolver::new(Linearity::new(0.5, density));
            let solver = Solver::new(&g, &resolver);
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
            let resolver = LinearityResolver::new(Linearity::default());
            let mut solver = Solver::new(&g, &resolver);
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

        let resolver = LinearityResolver::new(Linearity::default());
        let solver = Solver::new(&g, &resolver);
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

        let resolver = LinearityResolver::new(Linearity::default());
        let solver = Solver::new(&g, &resolver);
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

        let resolver = LinearityResolver::new(Linearity::new(0.2, 0.0));
        let solver = Solver::new(&g, &resolver).with_grant(key, dash);
        let solution = solver.fill(&mission, &[key], &Rng::new(3)).unwrap();

        assert_eq!(solution.traces.len(), 1);
        let t = &solution.traces[0];
        assert_eq!(t.item, key);
        assert!(t.candidates > 0);
        assert_eq!(t.locality, 0.2, "the dial actually in force is recorded");
        assert!(
            t.distance_to_lock.is_some(),
            "this key gates something, so distance is meaningful"
        );
    }

    #[test]
    fn items_that_gate_nothing_are_placed_freely() {
        let (g, rooms) = chain_world(5);
        let mission = mission_with_locations(&g, &rooms);
        let resolver = LinearityResolver::new(Linearity::default());
        // No grants at all — pure treasure.
        let solver = Solver::new(&g, &resolver);
        let items = [item("gold"), item("gem")];
        let solution = solver.fill(&mission, &items, &Rng::new(9)).unwrap();
        assert_eq!(solution.placements.len(), 2);
        for t in &solution.traces {
            assert_eq!(t.distance_to_lock, None, "nothing to be near");
        }
    }
}
