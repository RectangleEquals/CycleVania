//! **L2's data model** — the mission graph, the rule grammar edges are gated by, and the accessibility
//! analysis everything else rests on.
//!
//! # The mission graph is not the scope graph
//!
//! [`NodeGraph`](crate::node) says which rooms are *next to* each other. The mission graph says which
//! rooms you can *get to*, and what that costs. They deliberately differ: two adjacent rooms with a
//! locked door between them are neighbours in space and worlds apart in progression, and conflating
//! the two is how a generator ends up unable to reason about gating at all.
//!
//! So L2 takes the spatial adjacency as its starting topology, then decides what each connection
//! *requires*.
//!
//! # Spheres: the shape of progression
//!
//! Accessibility is not a single set — it is a sequence. With nothing, some rooms are accessible; the
//! items in them grant unlocks; those open more rooms; and so on. Each round is a **sphere**
//! ([`Sphere`]), and the sphere sequence *is* the progression structure:
//!
//! ```text
//! sphere 0: start, hall            (nothing needed)
//! sphere 1: vault, ledge           (after the bronze key found in sphere 0)
//! sphere 2: treasury                (after the dash found in sphere 1)
//! ```
//!
//! A world whose spheres are all size 1 is a corridor. One with a single enormous sphere is wide open.
//! Everything the linearity dials do shows up here, which makes spheres the natural thing to assert
//! against in tests and to show a dev in the editor.
//!
//! Computing them is a fixed point: sweep for what is accessible, collect what is there, sweep again,
//! until nothing new appears.

use crate::node::{Node, NodeGraph, NodeKind};
use crate::object::ObjectId;
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use crate::unlock::GrantMap;
use crate::Handle;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

// ---------------------------------------------------------------------------------------------
// Rule grammar
// ---------------------------------------------------------------------------------------------

/// A predicate over the player's state — what it takes to cross an edge or satisfy a placement.
///
/// ▶ This is the first cut of the `Rule` grammar the design left open. It is deliberately small: every
/// variant exists because the solver evaluates it, and combinators are here because gating genuinely
/// composes ("the dash **or** the grapple"). Anything speculative was left out — a grammar invented
/// ahead of its consumer fits nothing.
///
/// The shape is chosen so M16's checker can validate a script's rule expressions structurally, and so
/// the editor can render one as a tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Rule {
    /// Always satisfied — an open passage. The default: connections are open until L2 gates them.
    #[default]
    Always,
    /// Never satisfied. Useful as a placeholder for "sealed until L3 decides".
    Never,
    /// The player holds this unlock.
    Has(ObjectId),
    /// Every sub-rule holds.
    All(Vec<Rule>),
    /// At least one sub-rule holds — genuine alternate routes.
    Any(Vec<Rule>),
    /// The sub-rule does not hold.
    Not(Box<Rule>),
}

impl Rule {
    /// Requires holding a unlock.
    pub fn has(unlock: ObjectId) -> Rule {
        Rule::Has(unlock)
    }

    /// Requires all of a set of unlocks.
    pub fn all_of(unlocks: impl IntoIterator<Item = ObjectId>) -> Rule {
        let rules: Vec<Rule> = unlocks.into_iter().map(Rule::Has).collect();
        match rules.len() {
            0 => Rule::Always,
            1 => rules.into_iter().next().expect("length checked"),
            _ => Rule::All(rules),
        }
    }

    /// Requires any one of a set of unlocks.
    pub fn any_of(unlocks: impl IntoIterator<Item = ObjectId>) -> Rule {
        let rules: Vec<Rule> = unlocks.into_iter().map(Rule::Has).collect();
        match rules.len() {
            0 => Rule::Never, // "any of nothing" is unsatisfiable, not free
            1 => rules.into_iter().next().expect("length checked"),
            _ => Rule::Any(rules),
        }
    }

    /// Is this satisfied by a set of held unlocks?
    pub fn is_satisfied(&self, held: &BTreeSet<ObjectId>) -> bool {
        match self {
            Rule::Always => true,
            Rule::Never => false,
            Rule::Has(c) => held.contains(c),
            Rule::All(rules) => rules.iter().all(|r| r.is_satisfied(held)),
            Rule::Any(rules) => rules.iter().any(|r| r.is_satisfied(held)),
            Rule::Not(r) => !r.is_satisfied(held),
        }
    }

    /// Is this trivially satisfied regardless of state?
    pub fn is_open(&self) -> bool {
        matches!(self, Rule::Always)
    }

    /// Every unlock mentioned anywhere in the rule.
    ///
    /// What the solver needs to know which items gate this edge — and therefore which locks a key is
    /// "for" when [`crate::solver`] applies the locality dial.
    pub fn unlocks(&self) -> BTreeSet<ObjectId> {
        let mut out = BTreeSet::new();
        self.collect_unlocks(&mut out);
        out
    }

    fn collect_unlocks(&self, out: &mut BTreeSet<ObjectId>) {
        match self {
            Rule::Always | Rule::Never => {}
            Rule::Has(c) => {
                out.insert(*c);
            }
            Rule::All(rules) | Rule::Any(rules) => {
                for r in rules {
                    r.collect_unlocks(out);
                }
            }
            Rule::Not(r) => r.collect_unlocks(out),
        }
    }

    /// How deeply nested this rule is — a cheap complexity measure for the trace and for bounding
    /// pathological script-authored rules.
    pub fn depth(&self) -> u32 {
        match self {
            Rule::Always | Rule::Never | Rule::Has(_) => 1,
            Rule::All(rules) | Rule::Any(rules) => {
                1 + rules.iter().map(Rule::depth).max().unwrap_or(0)
            }
            Rule::Not(r) => 1 + r.depth(),
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rule::Always => write!(f, "open"),
            Rule::Never => write!(f, "sealed"),
            Rule::Has(c) => write!(f, "{c}"),
            Rule::All(rules) => {
                write!(f, "(")?;
                for (i, r) in rules.iter().enumerate() {
                    if i > 0 {
                        write!(f, " and ")?;
                    }
                    write!(f, "{r}")?;
                }
                write!(f, ")")
            }
            Rule::Any(rules) => {
                write!(f, "(")?;
                for (i, r) in rules.iter().enumerate() {
                    if i > 0 {
                        write!(f, " or ")?;
                    }
                    write!(f, "{r}")?;
                }
                write!(f, ")")
            }
            Rule::Not(r) => write!(f, "not {r}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Locations
// ---------------------------------------------------------------------------------------------

/// A place an item can be put — a slot the schedule planned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocationId(pub u32);

impl fmt::Display for LocationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "loc[{}]", self.0)
    }
}

/// Where a location is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    /// The scope holding it.
    pub scope: Handle<Node>,
    /// Which slot within that scope.
    pub slot: u32,
}

// ---------------------------------------------------------------------------------------------
// Edges and the graph
// ---------------------------------------------------------------------------------------------

/// A traversable connection between scopes, and what it costs.
#[derive(Clone, Debug, PartialEq)]
pub struct MissionEdge {
    /// Where it starts.
    pub from: Handle<Node>,
    /// Where it leads.
    pub to: Handle<Node>,
    /// What the player must hold to cross.
    pub rule: Rule,
    /// Can it be crossed back the other way?
    ///
    /// A one-way edge is a **commit**: everything behind it becomes unreachable unless another route
    /// exists. M10's un-softlockable pass is what proves that never strands the goal.
    pub reversible: bool,
    /// Was this added to create a loop, rather than being part of the base topology?
    pub is_shortcut: bool,
}

impl MissionEdge {
    /// An open, reversible connection.
    pub fn open(from: Handle<Node>, to: Handle<Node>) -> Self {
        MissionEdge {
            from,
            to,
            rule: Rule::Always,
            reversible: true,
            is_shortcut: false,
        }
    }

    /// A connection gated by a rule.
    pub fn gated(from: Handle<Node>, to: Handle<Node>, rule: Rule) -> Self {
        MissionEdge {
            from,
            to,
            rule,
            reversible: true,
            is_shortcut: false,
        }
    }

    /// Mark this edge one-way.
    pub fn one_way(mut self) -> Self {
        self.reversible = false;
        self
    }

    /// Mark this edge a shortcut (a loop-closing addition).
    pub fn shortcut(mut self) -> Self {
        self.is_shortcut = true;
        self
    }

    /// Is this edge gated at all?
    pub fn is_gated(&self) -> bool {
        !self.rule.is_open()
    }
}

/// One round of progression: what became accessible, and what it yielded.
#[derive(Clone, Debug, PartialEq)]
pub struct Sphere {
    /// How deep this sphere is; sphere 0 needs nothing.
    pub index: u32,
    /// Scopes that became accessible in this round.
    pub scopes: Vec<Handle<Node>>,
    /// Locations that became available in this round.
    pub locations: Vec<LocationId>,
    /// Unlocks obtained from those locations, opening the next sphere.
    pub granted: Vec<ObjectId>,
}

/// The outcome of a reachability sweep.
#[derive(Clone, Debug, PartialEq)]
pub struct Accessibility {
    /// Every accessible scope.
    pub scopes: BTreeSet<Handle<Node>>,
    /// Every accessible location.
    pub locations: BTreeSet<LocationId>,
    /// Every unlock obtainable.
    pub held: BTreeSet<ObjectId>,
    /// The progression, round by round.
    pub spheres: Vec<Sphere>,
}

impl Accessibility {
    /// Is a scope accessible?
    pub fn accessible(&self, scope: Handle<Node>) -> bool {
        self.scopes.contains(&scope)
    }

    /// How many rounds of progression this world has — a direct measure of how gated it is.
    pub fn depth(&self) -> u32 {
        self.spheres.len() as u32
    }

    /// Which sphere a scope first became accessible in.
    pub fn sphere_of(&self, scope: Handle<Node>) -> Option<u32> {
        self.spheres
            .iter()
            .find(|s| s.scopes.contains(&scope))
            .map(|s| s.index)
    }
}

/// **The mission graph** — the world as progression rather than as space.
#[derive(Clone, Debug, PartialEq)]
pub struct MissionGraph {
    start: Handle<Node>,
    /// Where the world is considered complete. The un-softlockable guarantee (M10) is stated against
    /// this: from every accessible state, *the goal* must stay accessible.
    goal: Option<Handle<Node>>,
    /// Scopes from which a stranded player can get back — a warp, a checkpoint, a hub return.
    ///
    /// Reaching one is what makes an otherwise-trapping one-way transition safe, which is exactly how
    /// real games handle it (Metroid's elevators, a dungeon's warp-out pedestal).
    recovery: BTreeSet<Handle<Node>>,
    edges: Vec<MissionEdge>,
    locations: BTreeMap<LocationId, Location>,
    /// Adjacency built from `edges`, rebuilt whenever they change.
    adjacency: BTreeMap<Handle<Node>, Vec<usize>>,
    /// The most connections a scope may have. Absent means unlimited, which is the normal case.
    ///
    /// **Enforced in [`add_edge`](MissionGraph::add_edge) rather than trusted to callers.** A cap is
    /// declared by something that already ran — a spine promising a dead-end treasury — and has to
    /// survive every pass that comes later. Making each pass remember to check would work until the
    /// first one forgot, and the failure would be a shipped world with the wrong shape.
    ///
    /// ▶ Whenever `MissionGraph` itself becomes serializable, **this field must go with it**: a cap is
    /// a promise, and a round-trip that drops it would let the next pass widen a dead end.
    degree_caps: BTreeMap<Handle<Node>, u32>,
    /// Scopes the generator must not put content in — the host owns their interiors.
    ///
    /// Enforced in [`add_location`](MissionGraph::add_location) for the same reason as `degree_caps`:
    /// an exclusion declared once has to survive every pass that comes after it, and the only way to
    /// guarantee that is to make the graph refuse rather than ask each pass to check.
    ///
    /// ▶ Must be serialized alongside `degree_caps` when the graph becomes serializable.
    content_excluded: BTreeSet<Handle<Node>>,
}

impl MissionGraph {
    /// An empty graph rooted at `start`.
    pub fn new(start: Handle<Node>) -> Self {
        MissionGraph {
            start,
            goal: None,
            recovery: BTreeSet::new(),
            edges: Vec::new(),
            locations: BTreeMap::new(),
            adjacency: BTreeMap::new(),
            degree_caps: BTreeMap::new(),
            content_excluded: BTreeSet::new(),
        }
    }

    /// Move where the run begins.
    ///
    /// Exists so a spine slot declared [`SlotRole::Start`](crate::spine::SlotRole::Start) can *be* the
    /// start rather than merely claim to be. A world always has one — the graph cannot be built
    /// without it — so this relocates it, never introduces it.
    pub fn set_start(&mut self, scope: Handle<Node>) -> &mut Self {
        self.start = scope;
        self
    }

    /// Forbid the generator from placing content in a scope.
    ///
    /// The scope keeps its connections and stays part of progression; what it loses is *contents*. No
    /// item location can be added to it, so nothing can be found there and nothing can gate on
    /// anything inside it. The host furnishes it however it likes.
    ///
    /// L1 has to be told separately — pass the same scopes to
    /// [`Scheduler::excluding`](crate::schedule::Scheduler::excluding) — because scheduling runs
    /// against the scope graph and never sees this one.
    pub fn exclude_content(&mut self, scope: Handle<Node>) -> &mut Self {
        self.content_excluded.insert(scope);
        self
    }

    /// Is the generator barred from placing content here?
    pub fn excludes_content(&self, scope: Handle<Node>) -> bool {
        self.content_excluded.contains(&scope)
    }

    /// Every scope the generator must leave empty, in handle order.
    pub fn content_excluded_scopes(&self) -> &BTreeSet<Handle<Node>> {
        &self.content_excluded
    }

    /// Cap how many connections a scope may have.
    ///
    /// This is how "the treasury is a dead end" survives `cycle_density`: once declared, no later pass
    /// can exceed it, because [`add_edge`](Self::add_edge) refuses rather than each pass being asked to
    /// remember. Capping below the current degree does **not** remove existing edges — it freezes the
    /// scope where it is.
    pub fn set_degree_cap(&mut self, scope: Handle<Node>, max: u32) -> &mut Self {
        self.degree_caps.insert(scope, max);
        self
    }

    /// The declared cap for a scope, if any.
    pub fn degree_cap(&self, scope: Handle<Node>) -> Option<u32> {
        self.degree_caps.get(&scope).copied()
    }

    /// How many edges touch a scope.
    pub fn degree(&self, scope: Handle<Node>) -> u32 {
        self.edges
            .iter()
            .filter(|e| e.from == scope || e.to == scope)
            .count() as u32
    }

    /// Would adding an edge between these two exceed either one's declared cap?
    pub fn would_exceed_cap(&self, a: Handle<Node>, b: Handle<Node>) -> bool {
        [a, b].iter().any(|h| {
            self.degree_cap(*h)
                .is_some_and(|cap| self.degree(*h) >= cap)
        })
    }

    /// Set where the world is considered complete.
    pub fn set_goal(&mut self, scope: Handle<Node>) -> &mut Self {
        self.goal = Some(scope);
        self
    }

    /// Where the world is considered complete, if declared.
    pub fn goal(&self) -> Option<Handle<Node>> {
        self.goal
    }

    /// Mark a scope as a recovery point — reaching it un-strands a player.
    pub fn add_recovery(&mut self, scope: Handle<Node>) -> &mut Self {
        self.recovery.insert(scope);
        self
    }

    /// The recovery points, in handle order.
    pub fn recovery_points(&self) -> &BTreeSet<Handle<Node>> {
        &self.recovery
    }

    /// Build the base topology from a scope graph's **spatial adjacency**.
    ///
    /// Every connection starts open; gating is L2's decision, made later. Building on the spatial
    /// graph rather than inventing a topology is what keeps the mission structure and the eventual
    /// geometry describing the same world.
    pub fn from_scopes(graph: &NodeGraph, start: Handle<Node>) -> Self {
        let mut mission = MissionGraph::new(start);
        mission.connect_scopes(graph);
        mission
    }

    /// Add the scope graph's spatial adjacency to an existing mission graph, **respecting degree
    /// caps**. Returns how many edges were added.
    ///
    /// Separate from [`from_scopes`](Self::from_scopes) because a spined pipeline needs the other
    /// order: the spine seeds its guaranteed structure into an empty graph *first*, declares its caps,
    /// and only then is the free-form adjacency poured in around it. Pouring first and constraining
    /// afterwards cannot work — a cap is forward-looking, and edges already placed would have to be
    /// torn out, which would invalidate every index the solver and the softlock pass hold.
    pub fn connect_scopes(&mut self, graph: &NodeGraph) -> usize {
        let mut added = 0;
        // Deterministic: walk order, and each node's neighbours in insertion order. Each undirected
        // spatial link becomes one edge, deduplicated by only taking pairs once.
        for scope in graph.walk() {
            let Some(node) = graph.get(scope) else {
                continue;
            };
            if node.kind() != NodeKind::Space {
                continue;
            }
            for peer in node.neighbors() {
                if scope < *peer
                    && !self.connects(scope, *peer)
                    && self.add_edge(MissionEdge::open(scope, *peer)).is_some()
                {
                    added += 1;
                }
            }
        }
        added
    }

    /// Where the player begins.
    pub fn start(&self) -> Handle<Node> {
        self.start
    }

    /// Every edge, in insertion order.
    pub fn edges(&self) -> &[MissionEdge] {
        &self.edges
    }

    /// Add an edge, unless a declared degree cap refuses it.
    ///
    /// Returns the new edge's index, or `None` when either endpoint is already at its cap. The
    /// `Option` is the point: an edge request is a *request*, and a graph carrying structural promises
    /// is entitled to decline. Passes that must not be refused — a spine wiring its own guaranteed
    /// adjacency — should run before the caps are set, which is the order
    /// [`SpineInstantiator`](crate::spine::SpineInstantiator) uses.
    pub fn add_edge(&mut self, edge: MissionEdge) -> Option<usize> {
        if self.would_exceed_cap(edge.from, edge.to) {
            return None;
        }
        let index = self.edges.len();
        self.adjacency.entry(edge.from).or_default().push(index);
        if edge.reversible {
            self.adjacency.entry(edge.to).or_default().push(index);
        }
        self.edges.push(edge);
        Some(index)
    }

    /// Make a one-way edge two-way. Returns whether anything changed.
    ///
    /// The blunt repair for a softlock: a commit you can undo is not a commit.
    pub fn make_reversible(&mut self, index: usize) -> bool {
        match self.edges.get_mut(index) {
            Some(edge) if !edge.reversible => {
                edge.reversible = true;
                let to = edge.to;
                self.adjacency.entry(to).or_default().push(index);
                true
            }
            _ => false,
        }
    }

    /// Make a two-way edge one-way, oriented `from → to`. Returns whether anything changed.
    pub fn make_one_way(&mut self, index: usize) -> bool {
        let Some(edge) = self.edges.get(index) else {
            return false;
        };
        if !edge.reversible {
            return false;
        }
        let to = edge.to;
        self.edges[index].reversible = false;
        // The destination can no longer walk back along this edge.
        if let Some(list) = self.adjacency.get_mut(&to) {
            list.retain(|i| *i != index);
        }
        true
    }

    /// Replace an edge's rule — how L2 gates a connection.
    pub fn gate_edge(&mut self, index: usize, rule: Rule) -> bool {
        match self.edges.get_mut(index) {
            Some(edge) => {
                edge.rule = rule;
                true
            }
            None => false,
        }
    }

    /// Register a location, unless its scope is barred from holding content.
    ///
    /// Returns whether it was added. Refusal is the point: a scope declared empty stays empty however
    /// many later passes try to fill it, without any of them having to know why.
    pub fn add_location(&mut self, id: LocationId, location: Location) -> bool {
        if self.excludes_content(location.scope) {
            return false;
        }
        self.locations.insert(id, location);
        true
    }

    /// Every location, in id order.
    pub fn locations(&self) -> impl Iterator<Item = (LocationId, Location)> + '_ {
        self.locations.iter().map(|(id, l)| (*id, *l))
    }

    /// How many locations exist.
    pub fn location_count(&self) -> usize {
        self.locations.len()
    }

    /// Locations inside a scope, in id order.
    pub fn locations_in(&self, scope: Handle<Node>) -> impl Iterator<Item = LocationId> + '_ {
        self.locations
            .iter()
            .filter(move |(_, l)| l.scope == scope)
            .map(|(id, _)| *id)
    }

    /// Edges that are gated on a unlock — the "locks" a key opens.
    pub fn locks_for(&self, unlock: ObjectId) -> impl Iterator<Item = &MissionEdge> + '_ {
        self.edges
            .iter()
            .filter(move |e| e.rule.unlocks().contains(&unlock))
    }

    /// **Sweep**: what is accessible, given a starting unlock set and what sits at each location.
    ///
    /// A fixed point rather than a single traversal: reaching a room may yield an item that opens
    /// another room, so the search repeats until a round adds nothing. Each round is a [`Sphere`].
    pub fn sweep(
        &self,
        initial: &BTreeSet<ObjectId>,
        placements: &BTreeMap<LocationId, ObjectId>,
        grants: &GrantMap,
    ) -> Accessibility {
        self.sweep_from(self.start, initial, placements, grants)
    }

    /// A sweep starting somewhere other than the world's start.
    ///
    /// This is what the un-softlockable analysis needs: "the player has just dropped through a one-way
    /// transition into `origin` holding only these unlocks — what can they still reach?"
    pub fn sweep_from(
        &self,
        origin: Handle<Node>,
        initial: &BTreeSet<ObjectId>,
        placements: &BTreeMap<LocationId, ObjectId>,
        grants: &GrantMap,
    ) -> Accessibility {
        let mut held = initial.clone();
        let mut scopes: BTreeSet<Handle<Node>> = BTreeSet::new();
        let mut locations: BTreeSet<LocationId> = BTreeSet::new();
        let mut spheres: Vec<Sphere> = Vec::new();

        loop {
            let round_scopes = self.traverse_from(origin, &held);
            let new_scopes: Vec<Handle<Node>> = round_scopes
                .iter()
                .filter(|s| !scopes.contains(s))
                .copied()
                .collect();
            let new_locations: Vec<LocationId> = new_scopes
                .iter()
                .flat_map(|s| self.locations_in(*s))
                .filter(|l| !locations.contains(l))
                .collect();

            // Whatever sits in the newly opened rooms is now obtainable.
            let mut granted: Vec<ObjectId> = Vec::new();
            for loc in &new_locations {
                if let Some(item) = placements.get(loc) {
                    if let Some(unlocks) = grants.get(item) {
                        for u in unlocks {
                            if !held.contains(u) {
                                granted.push(*u);
                            }
                        }
                    }
                }
            }
            granted.sort();
            granted.dedup();

            let progressed = !new_scopes.is_empty() || !granted.is_empty();
            if !progressed {
                break;
            }

            scopes.extend(&new_scopes);
            locations.extend(&new_locations);
            held.extend(&granted);
            spheres.push(Sphere {
                index: spheres.len() as u32,
                scopes: new_scopes,
                locations: new_locations,
                granted,
            });
        }

        Accessibility {
            scopes,
            locations,
            held,
            spheres,
        }
    }

    /// One breadth-first traversal with a fixed unlock set.
    ///
    /// Deterministic: a `VecDeque` frontier and edges visited in index order, so the same graph and
    /// unlocks always yield the same set — and, more importantly, the same *sphere boundaries*.
    pub fn traverse(&self, held: &BTreeSet<ObjectId>) -> BTreeSet<Handle<Node>> {
        self.traverse_from(self.start, held)
    }

    /// One breadth-first traversal from an arbitrary origin with a fixed unlock set.
    pub fn traverse_from(
        &self,
        origin: Handle<Node>,
        held: &BTreeSet<ObjectId>,
    ) -> BTreeSet<Handle<Node>> {
        let mut seen: BTreeSet<Handle<Node>> = BTreeSet::new();
        let mut queue: VecDeque<Handle<Node>> = VecDeque::new();
        seen.insert(origin);
        queue.push_back(origin);

        while let Some(at) = queue.pop_front() {
            let Some(edge_indices) = self.adjacency.get(&at) else {
                continue;
            };
            for &i in edge_indices {
                let edge = &self.edges[i];
                // A reversible edge may be walked from either end; a one-way only from `from`.
                let next = if edge.from == at {
                    edge.to
                } else if edge.reversible && edge.to == at {
                    edge.from
                } else {
                    continue;
                };
                if seen.contains(&next) || !edge.rule.is_satisfied(held) {
                    continue;
                }
                seen.insert(next);
                queue.push_back(next);
            }
        }
        seen
    }

    /// Hop distance from `origin` to every scope, ignoring gating.
    ///
    /// Ignoring gates is deliberate: this measures **spatial** separation, which is what the locality
    /// dial is about. Whether you currently *can* walk it is a different question, answered by
    /// [`MissionGraph::sweep`].
    pub fn distances_from(&self, origin: Handle<Node>) -> BTreeMap<Handle<Node>, u32> {
        let mut dist: BTreeMap<Handle<Node>, u32> = BTreeMap::new();
        let mut queue: VecDeque<Handle<Node>> = VecDeque::new();
        dist.insert(origin, 0);
        queue.push_back(origin);

        while let Some(at) = queue.pop_front() {
            let d = dist[&at];
            let Some(edge_indices) = self.adjacency.get(&at) else {
                continue;
            };
            for &i in edge_indices {
                let edge = &self.edges[i];
                let next = if edge.from == at { edge.to } else { edge.from };
                if let std::collections::btree_map::Entry::Vacant(slot) = dist.entry(next) {
                    slot.insert(d + 1);
                    queue.push_back(next);
                }
            }
        }
        dist
    }

    /// Is there an edge between these two scopes already?
    pub fn connects(&self, a: Handle<Node>, b: Handle<Node>) -> bool {
        self.edges
            .iter()
            .any(|e| (e.from == a && e.to == b) || (e.from == b && e.to == a))
    }

    /// How many edges were added as loop-closing shortcuts.
    pub fn shortcut_count(&self) -> usize {
        self.edges.iter().filter(|e| e.is_shortcut).count()
    }

    /// How many edges are gated.
    pub fn gated_count(&self) -> usize {
        self.edges.iter().filter(|e| e.is_gated()).count()
    }
}

// ---------------------------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------------------------

impl Serialize for Rule {
    fn serialize(&self, w: &mut Writer) {
        match self {
            Rule::Always => w.u8(0),
            Rule::Never => w.u8(1),
            Rule::Has(c) => {
                w.u8(2);
                w.write(c);
            }
            Rule::All(rules) => {
                w.u8(3);
                w.write(rules);
            }
            Rule::Any(rules) => {
                w.u8(4);
                w.write(rules);
            }
            Rule::Not(r) => {
                w.u8(5);
                w.write(r.as_ref());
            }
        }
    }
}

impl Deserialize for Rule {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(match r.u8()? {
            0 => Rule::Always,
            1 => Rule::Never,
            2 => Rule::Has(r.read()?),
            3 => Rule::All(r.read()?),
            4 => Rule::Any(r.read()?),
            5 => Rule::Not(Box::new(r.read()?)),
            _ => return Err(SerError::InvalidValue("unknown Rule tag")),
        })
    }
}

impl Serialize for LocationId {
    fn serialize(&self, w: &mut Writer) {
        w.u32(self.0);
    }
}

impl Deserialize for LocationId {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(LocationId(r.u32()?))
    }
}

impl Serialize for MissionEdge {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.from);
        w.write(&self.to);
        w.write(&self.rule);
        w.bool(self.reversible);
        w.bool(self.is_shortcut);
    }
}

impl Deserialize for MissionEdge {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(MissionEdge {
            from: r.read()?,
            to: r.read()?,
            rule: r.read()?,
            reversible: r.bool()?,
            is_shortcut: r.bool()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::{from_bytes, to_bytes};

    fn cap(name: &str) -> ObjectId {
        ObjectId::derived("unlock", name)
    }

    #[test]
    fn rules_evaluate_and_compose() {
        let dash = cap("dash");
        let grapple = cap("grapple");
        let held: BTreeSet<ObjectId> = [dash].into_iter().collect();

        assert!(Rule::Always.is_satisfied(&held));
        assert!(!Rule::Never.is_satisfied(&held));
        assert!(Rule::has(dash).is_satisfied(&held));
        assert!(!Rule::has(grapple).is_satisfied(&held));

        // Alternate routes: either traversal works.
        assert!(Rule::any_of([dash, grapple]).is_satisfied(&held));
        // Both required: not yet.
        assert!(!Rule::all_of([dash, grapple]).is_satisfied(&held));
        assert!(Rule::Not(Box::new(Rule::has(grapple))).is_satisfied(&held));
    }

    #[test]
    fn degenerate_rule_sets_mean_what_they_say() {
        // "All of nothing" is free; "any of nothing" is impossible. Getting these backwards would
        // silently open or seal every edge built from an empty list.
        assert_eq!(Rule::all_of([]), Rule::Always);
        assert_eq!(Rule::any_of([]), Rule::Never);
        // A single requirement does not need a wrapper.
        assert_eq!(Rule::all_of([cap("a")]), Rule::Has(cap("a")));
    }

    #[test]
    fn rules_report_what_they_depend_on() {
        let rule = Rule::All(vec![
            Rule::has(cap("dash")),
            Rule::Any(vec![Rule::has(cap("grapple")), Rule::has(cap("blink"))]),
        ]);
        assert_eq!(rule.unlocks().len(), 3);
        assert!(rule.unlocks().contains(&cap("blink")));
        assert_eq!(rule.depth(), 3);
        assert_eq!(Rule::Always.depth(), 1);
    }

    #[test]
    fn rules_read_well_in_a_trace() {
        let rule = Rule::Any(vec![Rule::has(cap("dash")), Rule::has(cap("grapple"))]);
        let text = rule.to_string();
        assert!(text.contains(" or "), "{text}");
        assert_eq!(Rule::Always.to_string(), "open");
        assert_eq!(Rule::Never.to_string(), "sealed");
    }

    /// A four-room chain: start → a → b → c, with a gate before `c`.
    fn chain() -> (NodeGraph, MissionGraph, Vec<Handle<Node>>) {
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "reach").unwrap();
        let area = g.add_child(reach, "area").unwrap();
        let rooms: Vec<Handle<Node>> = (0..4)
            .map(|i| g.add_child(area, format!("room_{i}")).unwrap())
            .collect();
        for w in rooms.windows(2) {
            g.connect(w[0], w[1]).unwrap();
        }
        let mission = MissionGraph::from_scopes(&g, rooms[0]);
        (g, mission, rooms)
    }

    #[test]
    fn the_base_topology_comes_from_spatial_adjacency() {
        let (_, mission, rooms) = chain();
        assert_eq!(mission.edges().len(), 3, "three links in a four-room chain");
        assert!(mission.connects(rooms[0], rooms[1]));
        assert!(!mission.connects(rooms[0], rooms[2]));
        assert_eq!(
            mission.gated_count(),
            0,
            "connections start open; gating is a later decision"
        );
    }

    #[test]
    fn a_gate_splits_the_world_into_spheres() {
        let (_, mut mission, rooms) = chain();
        let key = ObjectId::derived("item", "key");
        let dash = cap("dash");

        // Gate the last hop, and put the key in room 1.
        let last = mission.edges().len() - 1;
        mission.gate_edge(last, Rule::has(dash));
        mission.add_location(
            LocationId(0),
            Location {
                scope: rooms[1],
                slot: 0,
            },
        );

        let grants: GrantMap = [(key, BTreeSet::from([dash]))].into_iter().collect();
        let placements: BTreeMap<LocationId, ObjectId> =
            [(LocationId(0), key)].into_iter().collect();

        let r = mission.sweep(&BTreeSet::new(), &placements, &grants);
        assert!(
            r.accessible(rooms[3]),
            "the key is findable, so the gate opens"
        );
        assert_eq!(r.depth(), 2, "one sphere before the key, one after");
        assert_eq!(r.sphere_of(rooms[0]), Some(0));
        assert_eq!(r.sphere_of(rooms[3]), Some(1));
        assert_eq!(r.spheres[0].granted, vec![dash]);
    }

    #[test]
    fn a_key_behind_its_own_gate_is_unreachable() {
        // The circular dependency the solver exists to prevent. Sweeping must *notice*, not loop.
        let (_, mut mission, rooms) = chain();
        let key = ObjectId::derived("item", "key");
        let dash = cap("dash");
        let last = mission.edges().len() - 1;
        mission.gate_edge(last, Rule::has(dash));
        // Key placed *behind* the gate it opens.
        mission.add_location(
            LocationId(0),
            Location {
                scope: rooms[3],
                slot: 0,
            },
        );

        let grants: GrantMap = [(key, BTreeSet::from([dash]))].into_iter().collect();
        let placements: BTreeMap<LocationId, ObjectId> =
            [(LocationId(0), key)].into_iter().collect();

        let r = mission.sweep(&BTreeSet::new(), &placements, &grants);
        assert!(
            !r.accessible(rooms[3]),
            "unreachable, and the sweep terminates rather than spinning"
        );
        assert!(r.held.is_empty());
        assert_eq!(r.depth(), 1);
    }

    #[test]
    fn one_way_edges_are_only_walkable_forwards() {
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "reach").unwrap();
        let area = g.add_child(reach, "area").unwrap();
        let a = g.add_child(area, "a").unwrap();
        let b = g.add_child(area, "b").unwrap();

        let mut forward = MissionGraph::new(a);
        forward.add_edge(MissionEdge::open(a, b).one_way());
        assert!(forward
            .sweep(&BTreeSet::new(), &BTreeMap::new(), &BTreeMap::new())
            .accessible(b));

        // Starting at the far end, the same edge is impassable.
        let mut backward = MissionGraph::new(b);
        backward.add_edge(MissionEdge::open(a, b).one_way());
        assert!(!backward
            .sweep(&BTreeSet::new(), &BTreeMap::new(), &BTreeMap::new())
            .accessible(a));
    }

    #[test]
    fn distance_ignores_gating() {
        // Locality is about spatial separation; whether you can currently walk it is a different
        // question, and conflating them would make a gated-but-adjacent room look far away.
        let (_, mut mission, rooms) = chain();
        let last = mission.edges().len() - 1;
        mission.gate_edge(last, Rule::has(cap("dash")));
        let d = mission.distances_from(rooms[0]);
        assert_eq!(d[&rooms[3]], 3, "still three hops, gated or not");
        assert_eq!(d[&rooms[0]], 0);
    }

    #[test]
    fn sweeping_is_deterministic() {
        let (_, mut mission, rooms) = chain();
        let key = ObjectId::derived("item", "key");
        let dash = cap("dash");
        mission.gate_edge(2, Rule::has(dash));
        mission.add_location(
            LocationId(0),
            Location {
                scope: rooms[1],
                slot: 0,
            },
        );
        let grants: GrantMap = [(key, BTreeSet::from([dash]))].into_iter().collect();
        let placements: BTreeMap<LocationId, ObjectId> =
            [(LocationId(0), key)].into_iter().collect();

        let a = mission.sweep(&BTreeSet::new(), &placements, &grants);
        let b = mission.sweep(&BTreeSet::new(), &placements, &grants);
        assert_eq!(a, b);
    }

    #[test]
    fn rules_and_edges_round_trip() {
        let rule = Rule::All(vec![
            Rule::has(cap("dash")),
            Rule::Any(vec![
                Rule::has(cap("a")),
                Rule::Not(Box::new(Rule::has(cap("b")))),
            ]),
        ]);
        assert_eq!(from_bytes::<Rule>(&to_bytes(&rule)).unwrap(), rule);

        let (_, mission, _) = chain();
        let edge = mission.edges()[0].clone();
        assert_eq!(from_bytes::<MissionEdge>(&to_bytes(&edge)).unwrap(), edge);
    }
}
