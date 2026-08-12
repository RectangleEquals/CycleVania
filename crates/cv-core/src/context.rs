//! [`Context`] — the per-call lens the core hands into every [`Mechanic`](crate::Mechanic) callback.
//!
//! # It is the *only* argument
//!
//! A callback receives a `Context` and nothing else. That is a deliberate constraint rather than a
//! convenience: it means the complete set of things a mechanic can learn, and the complete set of
//! things it can ask for, is enumerable in one place. There is no ambient state to discover, no global
//! to reach for, and — crucially for the API-signature checker (M16) and the read-only header view
//! (M19) — no surface that varies per callback.
//!
//! # Four channels, with different authority
//!
//! | Channel | Who decides | Example |
//! |---|---|---|
//! | **Query** | reads committed fact | `ctx.count_realized(kind)` |
//! | **Assert** | the mechanic states ground truth | the *return value* of `footprint`/`constraints` |
//! | **Request** | the algorithm may grant, adapt, or deny | `ctx.request(Request::PreferSpacing(4.0))` |
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
//! # What this is at M07
//!
//! Enough for L0–L2: identity, randomness, scope reads, committed-state counts, and requests. The
//! geometric primitives (`raycast`, `reflect`, `sweep`, …) arrive at M11, and reactive dependency
//! tracking at M12 — which is why reads funnel through methods here rather than letting callers hold a
//! `&NodeGraph` directly. When M12 needs to record *what a call read*, there is one place to add it.

use crate::content::ContentRegistry;
use crate::mechanic::Request;
use crate::node::{Node, NodeGraph, NodeKind, NodeState};
use crate::object::ObjectId;
use crate::Handle;
use cv_determinism::Rng;

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
    /// Preferences collected during this call, in order.
    requests: Vec<Request>,
}

/// The read-only view of the world a context exposes.
struct WorldView<'a> {
    graph: &'a NodeGraph,
    registry: &'a ContentRegistry,
    /// Content already placed, by scope — what the committed-state queries count.
    placed: &'a [(Handle<Node>, ObjectId)],
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
            }),
            scope: None,
            rng: rng.fork(label),
            requests: Vec::new(),
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
            requests: Vec::new(),
        }
    }

    /// Point this context at a scope.
    pub fn at_scope(mut self, scope: Handle<Node>) -> Self {
        self.scope = Some(scope);
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
    /// Read-only is not a matter of trust: [`NodeGraph`]'s mutators are simply not reachable through a
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
    /// Committed state, so this re-derives correctly across backtracking. A `ProgressionAxis` (M12)
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

    // --- requests ---------------------------------------------------------------------------

    /// Ask for something. The algorithm may grant, adapt, or deny it.
    ///
    /// Returns nothing on purpose — a request that reported success would tempt a mechanic into
    /// branching on it, which would make behaviour depend on solver internals and on evaluation order.
    pub fn request(&mut self, request: Request) {
        self.requests.push(request);
    }

    /// The requests made during this call, in order.
    pub fn requests(&self) -> &[Request] {
        &self.requests
    }

    /// Take the collected requests, for the pipeline to weigh.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }
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
    fn requests_accumulate_in_order_and_report_nothing() {
        let (g, reg, placed) = world();
        let mut ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "t");
        // `request` returns (), so a mechanic cannot branch on whether it was granted.
        ctx.request(Request::PreferSpacing(4.0));
        ctx.request(Request::PreferScopeKind(NodeKind::Space));
        assert_eq!(ctx.requests().len(), 2);
        assert_eq!(ctx.requests()[0], Request::PreferSpacing(4.0));
        assert_eq!(ctx.take_requests().len(), 2);
        assert!(
            ctx.requests().is_empty(),
            "taking clears them for the next call"
        );
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
