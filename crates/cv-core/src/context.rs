//! [`Context`] — the per-call lens the core hands into every [`Mechanic`](crate::Mechanic) callback.
//!
//! # It is the *only* argument
//!
//! A callback receives a `Context` and nothing else. That is a deliberate constraint rather than a
//! convenience: it means the complete set of things a mechanic can learn, and the complete set of
//! things it can ask for, is enumerable in one place. There is no ambient state to discover, no global
//! to reach for, and — crucially for the API-signature checker and the read-only header view
//! — no surface that varies per callback.
//!
//! # Four channels, with different authority
//!
//! | Channel | Who decides | Example |
//! |---|---|---|
//! | **Query** | reads committed fact | `ctx.count_realized(kind)`, `ctx.raycast(o, d, r)` |
//! | **Assert** | the mechanic states ground truth | the *return value* of `footprint`/`constraints` |
//! | **Preference** | the algorithm may grant, adapt, or deny | `ctx.request(p)` — ▶ **M07** |
//! | **Randomness** | deterministic, label-addressed | `ctx.rng("placement")` |
//!
//! Note what is absent: **no setters.** A mechanic cannot write the graph, move a node, or force a
//! placement. Structure belongs to the algorithm ([`crate::node`]), and the only way to influence it is
//! to *ask*. That is what keeps the solvability guarantee the core's to make.
//!
//! # Queries read committed state
//!
//! The counting queries deliberately answer over **realized** scopes, not projected ones. Forecasts
//! get revised, so a value derived from them can silently disagree with the world after backtracking.
//! Counting committed state re-derives correctly because commitments are monotone — see the rule in
//! `01-core/pipeline.md`. `count_projected` exists too, spelled explicitly so that reading speculative
//! state is always a visible choice.
//!
//! # Spatial queries are queries, not decisions
//!
//! The primitives added at M11 — `raycast`, `sweep`, `slide_to_collision`, `line_of_sight`, `overlap`,
//! `reflect` — are **geometric only**. `raycast` reports what is *there*; it never reports whether that
//! thing blocks the caller, because blocking depends on what is travelling. Glass stops a bullet and
//! passes a laser: the glass knows that, the ray does not. A flow-selective question is therefore two
//! steps — walk `raycast_all` (sorted nearest-first) and stop at the first surface whose mechanic says
//! it blocks — and that split is what keeps `FlowKind` out of the geometry entirely.
//!
//! Geometry is optional on a context. Without it every primitive answers as an empty world would
//! (nothing hit, sight unobstructed, sweeps run their full length) rather than panicking, so a mechanic
//! written against a built world stays callable during L0–L2 when there is nothing to hit yet.
//!
//! # What this is at M11
//!
//! Identity, randomness, scope reads, committed-state counts, and the spatial primitives.
//! Reactive dependency tracking arrives at M12 — which is why reads funnel through methods here rather
//! than letting callers hold a `&NodeGraph` or a `&CoarseGeometry` directly. When M12 needs to record
//! *what a call read*, there is one place to add it, and the spatial reads are already inside it.

use crate::content::ContentRegistry;
use crate::floor::{FloorLadder, ScopeBounds};
use crate::geometry::{CoarseGeometry, ColliderId, Hit, Sweep};
use crate::node::{Node, NodeGraph, NodeKind, NodeState};
use crate::object::ObjectId;
use crate::query::Query;
use crate::trivalent::{self, Fidelity, Tolerances, Trivalent};
use crate::Handle;
use cv_determinism::{Aabb, Rng, Vec3};

/// The per-call generation lens.
///
/// Borrows the world read-only for the duration of one callback. Cheap to build — the pipeline makes
/// one per call rather than threading a long-lived object around.
pub struct Context<'a> {
    world: Option<WorldView<'a>>,
    /// The scope this call is about, when there is one.
    scope: Option<Handle<Node>>,
    /// The stream this call may draw from, forked to the call site.
    rng: Rng,
    /// How real the geometry is for this call.
    fidelity: Fidelity,
    /// The bounded error at each rung, and the project floor beneath them.
    tolerances: Tolerances,
}

/// The read-only view of the world a context exposes.
struct WorldView<'a> {
    graph: &'a NodeGraph,
    registry: &'a ContentRegistry,
    /// Content already placed, by scope — what the committed-state queries count.
    placed: &'a [(Handle<Node>, ObjectId)],
    /// The coarse boxes the spatial primitives run against. Absent until something builds them,
    /// in which case every primitive answers "nothing there" rather than refusing to run.
    geometry: Option<&'a CoarseGeometry>,
    floors: Option<&'a FloorLadder>,
}

impl<'a> Context<'a> {
    /// A context over a world.
    ///
    /// `label` seeds the forked RNG stream, so two different call sites never share randomness and the
    /// same call site is reproducible regardless of evaluation order.
    pub fn new(
        graph: &'a NodeGraph,
        registry: &'a ContentRegistry,
        placed: &'a [(Handle<Node>, ObjectId)],
        rng: &Rng,
        label: &str,
    ) -> Self {
        Context {
            world: Some(WorldView {
                graph,
                registry,
                placed,
                geometry: None,
                floors: None,
            }),
            scope: None,
            rng: rng.fork(label),
            fidelity: Fidelity::Envelope,
            tolerances: Tolerances::default(),
        }
    }

    /// A context with no world behind it.
    ///
    /// For unit-testing a mechanic's defaults and for callbacks made before any world exists. Every
    /// query answers emptily rather than panicking, so a mechanic written against a real world does
    /// not need a special case to be testable.
    pub fn detached() -> Self {
        Context {
            world: None,
            scope: None,
            rng: Rng::new(0),
            fidelity: Fidelity::Envelope,
            tolerances: Tolerances::default(),
        }
    }

    /// Point this context at a scope.
    pub fn at_scope(mut self, scope: Handle<Node>) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Set which rung of the fidelity ladder this call is running at.
    pub fn at_fidelity(mut self, fidelity: Fidelity) -> Self {
        self.fidelity = fidelity;
        self
    }

    /// Use a project's tolerances rather than the metre-scale defaults.
    pub fn with_tolerances(mut self, tolerances: Tolerances) -> Self {
        self.tolerances = tolerances;
        self
    }

    /// How real the geometry currently is.
    ///
    /// ⚠ **Fidelity is what *exists*; detail is what a query *asks for*.** A callback that confuses
    /// them ends up believing it received an answer the world could not yet supply.
    pub fn fidelity(&self) -> Fidelity {
        self.fidelity
    }

    /// The bounded error of this rung.
    ///
    /// The ladder is monotone, so this only ever **shrinks** as generation proceeds — which is the
    /// entire basis for *"a decision made outside the band at L2 cannot be overturned at L4"*.
    pub fn tolerance(&self) -> f64 {
        self.tolerances.at(self.fidelity)
    }

    /// Start a spatial query.
    ///
    /// Three independent axes — **what to trace** × **what to consider** × **what to report** — built
    /// as data and then run, so the whole shape is available to the VM and the editor before anything
    /// is traced. ⚠ The coherence guard `only_realized` is **on**; a hook that wants forecast content
    /// says so by name.
    pub fn query(&self) -> Query {
        Query::new()
    }

    /// Run a query against this context's geometry, at this context's fidelity.
    ///
    /// ⚠ Empty when there is no geometry, which is the same answer as *nothing was hit* — the two are
    /// separated by [`Context::geometry`] being `None`, not by a sentinel in the results.
    pub fn run(&self, query: &Query) -> Vec<Hit> {
        self.geometry()
            .map(|g| query.all(g, self.fidelity))
            .unwrap_or_default()
    }

    /// Is a measured distance within a limit, at this rung's confidence?
    ///
    /// ⚠ **Trivalent for METRIC questions.** Dual bounds answer set membership — *is this point in
    /// that region* — and this answers *is this ledge within 30 metres*, which is what every `Span`
    /// and budget comparison is really asking.
    ///
    /// An `AMBIGUOUS` result re-asks at the next rung. There is deliberately no marker for that: a
    /// marker would have to be stored, propagated and cleared, and each of those is a chance to leave
    /// one behind.
    pub fn within(&self, measured: f64, limit: f64) -> Trivalent {
        trivalent::within(measured, limit, self.tolerance())
    }

    /// Give this context the coarse geometry the spatial primitives run against.
    ///
    /// Optional on purpose: L0–L2 callbacks have no geometry to speak of, and a mechanic that asks
    /// anyway gets an empty world rather than a panic — the same forgiving shape
    /// [`detached`](Self::detached) has, for the same reason.
    pub fn with_geometry(mut self, geometry: &'a CoarseGeometry) -> Self {
        if let Some(world) = self.world.as_mut() {
            world.geometry = Some(geometry);
        }
        self
    }

    // --- identity ---------------------------------------------------------------------------

    /// The scope this call is about, if any.
    pub fn scope(&self) -> Option<Handle<Node>> {
        self.scope
    }

    /// The scope's node.
    pub fn scope_node(&self) -> Option<&Node> {
        let world = self.world.as_ref()?;
        world.graph.get(self.scope?)
    }

    /// The kind of scope this call is about.
    pub fn scope_kind(&self) -> Option<NodeKind> {
        self.scope_node().map(|n| n.kind())
    }

    /// The registry of everything that may exist.
    pub fn registry(&self) -> Option<&ContentRegistry> {
        self.world.as_ref().map(|w| w.registry)
    }

    /// Read-only access to the scope graph.
    ///
    /// Read-only is not a matter of trust: [`NodeGraph`]'s mutators are simply not accessible through a
    /// shared reference, so a mechanic cannot write structure even if it tried.
    pub fn graph(&self) -> Option<&NodeGraph> {
        self.world.as_ref().map(|w| w.graph)
    }

    // --- randomness -------------------------------------------------------------------------

    /// A forked stream for a named purpose.
    ///
    /// Label-addressed, so `ctx.rng("jitter")` yields the same stream no matter when it is called or
    /// what else the mechanic did first — the property that makes generation logic refactor-safe.
    pub fn rng(&self, label: &str) -> Rng {
        self.rng.fork(label)
    }

    // --- queries over committed state --------------------------------------------------------

    /// How many scopes of a kind are **realized**.
    ///
    /// Committed state, so this re-derives correctly across backtracking. A `ProgressionAxis`
    /// built on queries like this rolls back for free.
    pub fn count_realized(&self, kind: NodeKind) -> u32 {
        self.count_scopes(kind, NodeState::Realized)
    }

    /// How many scopes of a kind are at least `Reserved` — also monotone, so also safe to count.
    pub fn count_committed(&self, kind: NodeKind) -> u32 {
        match self.world.as_ref() {
            None => 0,
            Some(w) => w
                .graph
                .of_kind(kind)
                .filter(|(_, n)| n.state() >= NodeState::Reserved)
                .count() as u32,
        }
    }

    /// How many scopes of a kind are still **forecasts**.
    ///
    /// Spelled explicitly because counting speculative state is a real choice with real consequences:
    /// forecasts get revised, so a value derived from this can disagree with the world afterwards. Use
    /// it when you mean "how much is still unresolved", not as a stand-in for how big the world is.
    pub fn count_projected(&self, kind: NodeKind) -> u32 {
        self.count_scopes(kind, NodeState::Projected)
    }

    /// How many instances of a content id have been placed so far.
    pub fn count_placed(&self, content: ObjectId) -> u32 {
        match self.world.as_ref() {
            None => 0,
            Some(w) => w.placed.iter().filter(|(_, c)| *c == content).count() as u32,
        }
    }

    /// How many instances of a content id sit in this call's scope.
    pub fn count_placed_here(&self, content: ObjectId) -> u32 {
        match (self.world.as_ref(), self.scope) {
            (Some(w), Some(scope)) => w
                .placed
                .iter()
                .filter(|(s, c)| *s == scope && *c == content)
                .count() as u32,
            _ => 0,
        }
    }

    /// Has this content been placed anywhere yet?
    pub fn is_placed(&self, content: ObjectId) -> bool {
        self.count_placed(content) > 0
    }

    fn count_scopes(&self, kind: NodeKind, state: NodeState) -> u32 {
        match self.world.as_ref() {
            None => 0,
            Some(w) => w
                .graph
                .of_kind(kind)
                .filter(|(_, n)| n.state() == state)
                .count() as u32,
        }
    }

    // --- spatial primitives --------------------------------------------------------------

    /// The coarse geometry behind this call, if it has any.
    pub fn geometry(&self) -> Option<&CoarseGeometry> {
        self.world.as_ref().and_then(|w| w.geometry)
    }

    /// Give this context the floor ladder — **this is what takes spatial queries live at L2c**.
    ///
    /// Separate from [`Context::with_geometry`] because the two arrive at different moments: colliders
    /// exist as soon as anything is committed, but the bounds derived from them only exist once the
    /// L2a→L2c pass has run.
    pub fn with_floors(mut self, floors: &'a FloorLadder) -> Self {
        if let Some(world) = self.world.as_mut() {
            world.floors = Some(floors);
        }
        self
    }

    /// The dual bounds for a scope, once the ladder has run.
    ///
    /// ⚠ **`None` means "not known yet", never "nothing there".** A hook running before L2c gets no
    /// answer rather than a wrong one — the distinction `Trivalent` formalises at M06.
    pub fn bounds(&self, scope: Handle<Node>) -> Option<&ScopeBounds> {
        self.world
            .as_ref()
            .and_then(|w| w.floors)
            .and_then(|l| l.bounds(scope))
    }

    /// Could an occupant be standing here?
    ///
    /// ⚠ **This is the question M05a could not answer honestly**, and the reason it now returns three
    /// values. Two bounds produce three cases, and a bool had to collapse two of them together:
    ///
    /// | | Answer |
    /// |---|---|
    /// | inside the inner bound | `YES` — committed floor is there |
    /// | outside the outer bound | `NO` — and no later rung can put geometry there |
    /// | between them | `AMBIGUOUS` — the hull spans it, the floor does not |
    /// | the ladder has not run | `AMBIGUOUS` — **not** `NO`; nothing is known yet |
    ///
    /// That last row is the one a bool got wrong: *"not computed"* and *"nothing there"* are
    /// different facts, and answering `false` to both is how an optimistic bound becomes a lie.
    pub fn standable(&self, scope: Handle<Node>, p: Vec3) -> Trivalent {
        match self.bounds(scope) {
            None => Trivalent::Ambiguous,
            Some(b) if b.inner_contains(p) => Trivalent::Yes,
            Some(b) if !b.outer_contains(p) => Trivalent::No,
            Some(_) => Trivalent::Ambiguous,
        }
    }

    /// The first thing in the way.
    ///
    /// **Geometric, not semantic.** It reports what is *there*, never whether that thing blocks the
    /// caller — glass stops a bullet and passes a laser, and the glass knows that while the ray does
    /// not. For a flow-selective answer, walk [`raycast_all`](Self::raycast_all) and stop at the first
    /// surface whose mechanic says it blocks.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f64) -> Option<Hit> {
        self.geometry()?.raycast(origin, direction, max_distance)
    }

    /// Everything the ray meets, nearest first — the building block for flow-selective queries.
    pub fn raycast_all(&self, origin: Vec3, direction: Vec3, max_distance: f64) -> Vec<Hit> {
        self.geometry()
            .map(|g| g.raycast_all(origin, direction, max_distance))
            .unwrap_or_default()
    }

    /// Is the straight line between two points unobstructed by anything at all?
    ///
    /// Returns `true` with no geometry: "nothing is in the way" is the honest answer to an empty world,
    /// and it keeps a mechanic's logic identical whether or not geometry has been built yet.
    pub fn line_of_sight(&self, from: Vec3, to: Vec3) -> Trivalent {
        let Some(g) = self.geometry() else {
            // ⚠ No geometry is not "clear" — it is "not known yet".
            return Trivalent::Ambiguous;
        };
        if g.line_of_sight(from, to) {
            // ⚠ **Clear is definite, and it relies on one invariant**: a collider *bounds* its content
            // and is never bounded by it. Real geometry lives inside the box, so a ray that misses the
            // box misses everything the box will ever contain.
            Trivalent::Yes
        } else if self.fidelity == Fidelity::Geometry {
            Trivalent::No
        } else {
            // Blocked by a coarse box says the *box* is in the way. What ends up inside it may be
            // narrower, so the sightline may open at a later rung — re-ask there.
            Trivalent::Ambiguous
        }
    }

    /// Move a box until it touches something.
    pub fn sweep(&self, box_: Aabb, direction: Vec3, max_distance: f64) -> Sweep {
        match self.geometry() {
            Some(g) => g.sweep(box_, direction, max_distance),
            None => Sweep {
                distance: max_distance,
                end: box_.center() + direction * max_distance,
                hit: None,
            },
        }
    }

    /// Move a box until it touches something, then slide along the contact.
    pub fn slide_to_collision(&self, box_: Aabb, direction: Vec3, max_distance: f64) -> Sweep {
        match self.geometry() {
            Some(g) => g.slide_to_collision(box_, direction, max_distance),
            None => self.sweep(box_, direction, max_distance),
        }
    }

    /// Every collider intersecting a box.
    pub fn overlap(&self, bounds: Aabb) -> Vec<ColliderId> {
        self.geometry()
            .map(|g| g.overlap(bounds))
            .unwrap_or_default()
    }

    /// Reflect a direction off a normal — the mirror rule a deflective surface applies.
    ///
    /// Pure arithmetic, so it works on a detached context: a mechanic computing a bounce should not
    /// need a world to do it.
    pub fn reflect(&self, incoming: Vec3, normal: Vec3) -> Vec3 {
        CoarseGeometry::reflect(incoming, normal)
    }

    /// The surface tags at a hit.
    pub fn tags_at(&self, hit: &Hit) -> Vec<ObjectId> {
        self.geometry().map(|g| g.tags_at(hit)).unwrap_or_default()
    }

    /// Does the surface a hit landed on carry a tag?
    pub fn has_tag_at(&self, hit: &Hit, tag: ObjectId) -> bool {
        self.geometry().is_some_and(|g| g.has_tag_at(hit, tag))
    }

    // --- requests ---------------------------------------------------------------------------
    //
    // ▶ **M07 P07.** The channel is design-backed — `ctx.request(p: Ref<Preference>)`, *"ask the
    // solver for something, softly"* — but `Preference` does not exist yet. The pre-v0.1 `Request`
    // enum that used to sit here came from the mechanic seam and went with it at M04, rather than
    // being carried forward into a shape the design does not have.
}

#[cfg(test)]
mod tests {
    use super::*;
    use cv_determinism::Aabb;
    use cv_determinism::Vec3;

    fn world() -> (NodeGraph, ContentRegistry, Vec<(Handle<Node>, ObjectId)>) {
        let mut g = NodeGraph::new(1.0, 42);
        let reach = g.add_child(g.root(), "reach").unwrap();
        let area = g.add_child(reach, "area").unwrap();
        let spaces: Vec<_> = (0..3)
            .map(|i| g.add_child(area, format!("space_{i}")).unwrap())
            .collect();
        for h in [g.root(), reach, area] {
            g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(50.0)))
                .unwrap();
            g.advance(h, NodeState::Realized).unwrap();
        }
        // One realized, one reserved, one still a forecast.
        g.set_envelope(spaces[0], Aabb::new(Vec3::ZERO, Vec3::ONE))
            .unwrap();
        g.advance(spaces[0], NodeState::Realized).unwrap();
        g.set_envelope(spaces[1], Aabb::new(Vec3::ZERO, Vec3::ONE))
            .unwrap();
        g.advance(spaces[1], NodeState::Reserved).unwrap();

        let door = ObjectId::derived("actor", "door");
        let placed = vec![(spaces[0], door), (spaces[0], door), (spaces[1], door)];
        (g, ContentRegistry::new(), placed)
    }

    #[test]
    fn queries_distinguish_committed_from_speculative() {
        let (g, reg, placed) = world();
        let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "test");
        assert_eq!(ctx.count_realized(NodeKind::Space), 1);
        assert_eq!(
            ctx.count_committed(NodeKind::Space),
            2,
            "realized + reserved are both monotone"
        );
        assert_eq!(ctx.count_projected(NodeKind::Space), 1);
        assert_eq!(ctx.count_realized(NodeKind::Reach), 1);
    }

    #[test]
    fn placement_counts_are_scoped_or_global() {
        let (g, reg, placed) = world();
        let door = ObjectId::derived("actor", "door");
        let space_0 = g.of_kind(NodeKind::Space).next().unwrap().0;

        let global = Context::new(&g, &reg, &placed, &Rng::new(1), "t");
        assert_eq!(global.count_placed(door), 3);
        assert!(global.is_placed(door));
        assert_eq!(
            global.count_placed_here(door),
            0,
            "no scope set, so nothing is 'here'"
        );

        let here = Context::new(&g, &reg, &placed, &Rng::new(1), "t").at_scope(space_0);
        assert_eq!(here.count_placed_here(door), 2);
        assert_eq!(here.scope_kind(), Some(NodeKind::Space));
        assert!(!global.is_placed(ObjectId::derived("actor", "absent")));
    }

    #[test]
    fn rng_is_label_addressed_and_order_independent() {
        let (g, reg, placed) = world();
        let ctx = Context::new(&g, &reg, &placed, &Rng::new(7), "call");
        // Same label ⇒ same stream, regardless of what was drawn in between.
        let mut a = ctx.rng("jitter");
        let mut b = ctx.rng("other");
        let mut a_again = ctx.rng("jitter");
        assert_eq!(a.next_u64(), a_again.next_u64());
        assert_ne!(b.next_u64(), ctx.rng("jitter").next_u64());

        // A different call label gives a different stream for the same purpose.
        let other_call = Context::new(&g, &reg, &placed, &Rng::new(7), "different_call");
        assert_ne!(
            ctx.rng("jitter").next_u64(),
            other_call.rng("jitter").next_u64()
        );
    }

    #[test]
    fn spatial_primitives_answer_through_the_context() {
        use crate::geometry::{CoarseGeometry, Collider, Face};
        let (g, reg, placed) = world();
        let mut geometry = CoarseGeometry::new();
        let wall = ObjectId::derived("actor", "wall");
        let portalable = ObjectId::derived("surface", "portalable");
        geometry.add(
            Collider::new(
                wall,
                Aabb::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 2.0, 2.0)),
            )
            .tagged_face(Face::NegX, portalable),
        );

        let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "t").with_geometry(&geometry);
        let hit = ctx
            .raycast(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 10.0)
            .expect("the wall is in the way");
        assert_eq!(hit.owner, wall);
        assert_eq!(hit.distance, 2.0);
        assert!(
            ctx.has_tag_at(&hit, portalable),
            "tags read back through the context"
        );
        // ⚠ Blocked by a coarse box is not yet a definite "no": what ends up inside the box may be
        // narrower. Clear *is* definite, because a collider bounds its content.
        assert_eq!(
            ctx.line_of_sight(Vec3::new(0.0, 1.0, 1.0), Vec3::new(5.0, 1.0, 1.0)),
            Trivalent::Ambiguous
        );
        assert_eq!(
            ctx.line_of_sight(Vec3::new(0.0, 5.0, 1.0), Vec3::new(5.0, 5.0, 1.0)),
            Trivalent::Yes
        );
        assert_eq!(
            ctx.raycast_all(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 10.0)
                .len(),
            1
        );
        assert_eq!(
            ctx.overlap(Aabb::new(Vec3::ZERO, Vec3::splat(10.0))).len(),
            1
        );
    }

    #[test]
    fn spatial_primitives_are_geometric_and_never_decide_what_blocks() {
        // The property that keeps `FlowKind` out of the geometry: a ray reports what is *there*, and
        // the caller decides whether it stops them. Two mechanics can disagree about the same hit.
        use crate::geometry::{CoarseGeometry, Collider};
        let (g, reg, placed) = world();
        let glass = ObjectId::derived("actor", "glass");
        let mut geometry = CoarseGeometry::new();
        geometry.add(Collider::new(
            glass,
            Aabb::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 2.0, 2.0)),
        ));
        geometry.add(Collider::new(
            ObjectId::derived("actor", "stone"),
            Aabb::new(Vec3::new(5.0, 0.0, 0.0), Vec3::new(6.0, 2.0, 2.0)),
        ));

        let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "t").with_geometry(&geometry);
        let hits = ctx.raycast_all(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 20.0);
        assert_eq!(
            hits.len(),
            2,
            "the ray reports both, blocking is not its call"
        );

        // A bullet stops at the glass; a laser passes it and stops at the stone. Same ray, same hits.
        let bullet_stop = hits.iter().find(|h| h.owner == glass).unwrap();
        let laser_stop = hits.iter().find(|h| h.owner != glass).unwrap();
        assert_eq!(bullet_stop.distance, 2.0);
        assert_eq!(laser_stop.distance, 5.0);
    }

    #[test]
    fn without_geometry_the_primitives_answer_as_an_empty_world() {
        // A mechanic written against a built world must stay callable during L0–L2, when there is
        // nothing to hit yet — otherwise every mechanic needs a "has geometry?" branch.
        let (g, reg, placed) = world();
        let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "t");
        assert!(ctx.geometry().is_none());
        assert!(ctx.raycast(Vec3::ZERO, Vec3::X, 10.0).is_none());
        assert!(ctx.raycast_all(Vec3::ZERO, Vec3::X, 10.0).is_empty());
        assert_eq!(
            ctx.line_of_sight(Vec3::ZERO, Vec3::splat(9.0)),
            Trivalent::Ambiguous,
            "with no geometry at all the honest answer is 'not known yet', never 'clear'"
        );
        let mover = Aabb::from_center_extents(Vec3::ZERO, Vec3::splat(0.5));
        let sweep = ctx.sweep(mover, Vec3::X, 4.0);
        assert!(sweep.is_clear());
        assert_eq!(sweep.distance, 4.0);
        assert!(ctx.overlap(Aabb::new(Vec3::ZERO, Vec3::ONE)).is_empty());
    }

    #[test]
    fn reflect_works_without_a_world_at_all() {
        // Pure arithmetic: computing a bounce should not require geometry to have been built.
        let ctx = Context::detached();
        let r = ctx.reflect(Vec3::new(1.0, -1.0, 0.0).normalized(), Vec3::Y);
        assert!((r - Vec3::new(1.0, 1.0, 0.0).normalized()).length() < 1e-12);
    }

    #[test]
    fn a_detached_context_answers_emptily_rather_than_panicking() {
        let ctx = Context::detached();
        assert_eq!(ctx.count_realized(NodeKind::Space), 0);
        assert_eq!(ctx.count_placed(ObjectId::derived("actor", "door")), 0);
        assert!(ctx.scope().is_none());
        assert!(ctx.scope_node().is_none());
        assert!(ctx.graph().is_none());
        assert!(ctx.registry().is_none());
        // Randomness still works, so a mechanic's logic is testable without a world.
        assert_ne!(
            ctx.rng("a").clone().next_u64(),
            ctx.rng("b").clone().next_u64()
        );
    }
}
