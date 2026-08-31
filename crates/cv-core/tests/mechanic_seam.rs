//! M07's exit criterion, and the claim the whole milestone exists to make:
//!
//! > A pipeline written against [`Mechanic`] does not change when the implementation behind it changes
//! > from hand-written Rust to a VM.
//!
//! That is what breaks the core↔CVScript cycle. The core's API shape depends on what mechanics need to
//! express; CVScript wraps that API; so the API must be settled first — which means building the
//! pipeline against *something* before the language exists. This file proves the substitution is real
//! rather than hoped for.
//!
//! The method: one `analyze` function (a stand-in for L2), called twice — once over Rust fixtures,
//! once over a mechanic whose behaviour comes from a **data table it interprets at runtime**, the way
//! the bytecode VM will at M18. The two implementations share no code path. If the results match and
//! `analyze` is untouched between them, the seam holds.
//!
//! Note what `analyze` *cannot* do: it holds `&dyn Mechanic`, so it has no way to ask which kind of
//! implementation it received. The substitution is enforced by the type, not by discipline.

use cv_core::fixtures::{Deflective, Door, Glass, KeyItem, Ledge, MovementUnlock};
use cv_core::{
    Constraint, Constraints, ContentKind, ContentRegistry, Context, FlowKind, Mechanic,
    MechanicRegistry, NodeGraph, NodeKind, NodeState, ObjectId, Traversal, TraversalKind, Volume,
};
use cv_determinism::{Aabb, Rng, Vec3};

// ---------------------------------------------------------------------------------------------
// A stand-in for the VM-backed mechanics of M18
// ---------------------------------------------------------------------------------------------

/// The "compiled" form of a mechanic: data, not code.
///
/// This is the shape M18 produces — behaviour lifted out of Rust and into something the core
/// interprets. Whether the interpreter reads this struct or a `.cvb` bytecode buffer is an
/// implementation detail *below* the trait, which is precisely the point.
#[derive(Clone, Default)]
struct MechanicProgram {
    kind_tag: Option<ContentKind>,
    label: String,
    footprint: Option<Volume>,
    constraints: Vec<Constraint>,
    traversals: Vec<Traversal>,
    grants: Option<ObjectId>,
    blocks_flows: Vec<FlowKind>,
    reflect_about: Option<Vec3>,
    reflects_flows: Vec<FlowKind>,
}

/// A mechanic whose every answer is *interpreted from a program* rather than compiled into Rust.
struct VmBacked {
    program: MechanicProgram,
}

impl Mechanic for VmBacked {
    fn kind(&self) -> ContentKind {
        self.program.kind_tag.unwrap_or(ContentKind::Actor)
    }

    fn label(&self) -> &str {
        &self.program.label
    }

    fn footprint(&self, _ctx: &Context<'_>) -> Option<Volume> {
        self.program.footprint
    }

    fn constraints(&self, _ctx: &Context<'_>) -> Constraints {
        Constraints::of(self.program.constraints.iter().cloned())
    }

    fn affords(&self, _ctx: &Context<'_>) -> Vec<Traversal> {
        self.program.traversals.clone()
    }

    fn grants(&self, _ctx: &Context<'_>) -> Option<ObjectId> {
        self.program.grants
    }

    fn blocks(&self, _ctx: &Context<'_>, flow: FlowKind) -> bool {
        self.program.blocks_flows.contains(&flow)
    }

    fn redirects(&self, _ctx: &Context<'_>, flow: FlowKind, incoming: Vec3) -> Option<Vec3> {
        let normal = self.program.reflect_about?;
        if self.program.reflects_flows.contains(&flow) {
            Some(incoming.reflect(normal))
        } else {
            None
        }
    }
}

/// Ids shared by both registries.
struct Ids {
    door: ObjectId,
    ledge: ObjectId,
    key: ObjectId,
    dash: ObjectId,
    glass: ObjectId,
    mirror: ObjectId,
}

impl Ids {
    fn new() -> Self {
        Ids {
            door: ObjectId::derived("actor", "door_heavy"),
            ledge: ObjectId::derived("actor", "ledge"),
            key: ObjectId::derived("item", "key_bronze"),
            dash: ObjectId::derived("unlock", "blink_dash"),
            glass: ObjectId::derived("surface", "glass"),
            mirror: ObjectId::derived("surface", "deflective"),
        }
    }
}

/// The M07 world: behaviour written in Rust.
fn rust_mechanics(ids: &Ids) -> MechanicRegistry {
    let mut r = MechanicRegistry::new();
    r.register(ids.door, Box::new(Door::locked_by(ids.dash)));
    r.register(ids.ledge, Box::new(Ledge));
    r.register(ids.key, Box::new(KeyItem::granting(ids.dash)));
    r.register(
        ids.dash,
        Box::new(MovementUnlock::new("Blink Dash", TraversalKind::Blink)),
    );
    r.register(ids.glass, Box::new(Glass));
    r.register(ids.mirror, Box::new(Deflective::facing(Vec3::Z)));
    r
}

/// The M18 world: the same behaviour, interpreted from programs.
///
/// Written out by hand here; at M18 the compiler produces it from `.cvs` source.
fn vm_mechanics(ids: &Ids) -> MechanicRegistry {
    let mut r = MechanicRegistry::new();

    r.register(
        ids.door,
        Box::new(VmBacked {
            program: MechanicProgram {
                kind_tag: Some(ContentKind::Actor),
                label: "Door".into(),
                footprint: Some(Volume::with_clearance(
                    Aabb::from_center_extents(Vec3::ZERO, Vec3::new(1.0, 0.25, 1.5)),
                    0.5,
                )),
                constraints: vec![
                    Constraint::WithinScopeKind(NodeKind::Space),
                    Constraint::RequiresUnlock(ids.dash),
                ],
                traversals: vec![Traversal::gated(TraversalKind::Walk, [ids.dash])],
                ..Default::default()
            },
        }),
    );
    r.register(
        ids.ledge,
        Box::new(VmBacked {
            program: MechanicProgram {
                kind_tag: Some(ContentKind::Actor),
                label: "Ledge".into(),
                constraints: vec![Constraint::WithinScopeKind(NodeKind::Space)],
                traversals: vec![Traversal::open(TraversalKind::Jump).one_way()],
                ..Default::default()
            },
        }),
    );
    r.register(
        ids.key,
        Box::new(VmBacked {
            program: MechanicProgram {
                kind_tag: Some(ContentKind::Item),
                label: "KeyItem".into(),
                footprint: Some(Volume::cube(0.5)),
                constraints: vec![Constraint::WithinScopeKind(NodeKind::Space)],
                grants: Some(ids.dash),
                ..Default::default()
            },
        }),
    );
    r.register(
        ids.dash,
        Box::new(VmBacked {
            program: MechanicProgram {
                kind_tag: Some(ContentKind::Item),
                label: "Blink Dash".into(),
                traversals: vec![Traversal::open(TraversalKind::Blink)],
                ..Default::default()
            },
        }),
    );
    r.register(
        ids.glass,
        Box::new(VmBacked {
            program: MechanicProgram {
                kind_tag: Some(ContentKind::SurfaceProperty),
                label: "Glass".into(),
                blocks_flows: vec![FlowKind::Ballistic, FlowKind::Walking, FlowKind::Portal],
                ..Default::default()
            },
        }),
    );
    r.register(
        ids.mirror,
        Box::new(VmBacked {
            program: MechanicProgram {
                kind_tag: Some(ContentKind::SurfaceProperty),
                label: "Deflective".into(),
                blocks_flows: vec![
                    FlowKind::Walking,
                    FlowKind::Ballistic,
                    FlowKind::Funnel,
                    FlowKind::Portal,
                ],
                reflect_about: Some(Vec3::Z),
                reflects_flows: vec![FlowKind::Laser, FlowKind::Sight],
                ..Default::default()
            },
        }),
    );
    r
}

// ---------------------------------------------------------------------------------------------
// The pipeline — written once, run against both
// ---------------------------------------------------------------------------------------------

/// What a solver pass learns by interrogating mechanics.
#[derive(Debug, PartialEq)]
struct Analysis {
    /// `(label, footprint volume, constraint count)` per content, in id order.
    profiles: Vec<(String, Option<f64>, usize)>,
    /// Traversal edges the world affords, with what each costs.
    edges: Vec<(TraversalKind, Vec<ObjectId>, bool)>,
    /// Unlocks obtainable, and from what.
    grants: Vec<(ObjectId, ObjectId)>,
    /// Which flows each surface stops.
    blocked: Vec<(String, Vec<FlowKind>)>,
    /// Where a laser fired straight down ends up.
    laser_bounce: Option<Vec3>,
    /// One-way commits, which M10's un-softlockable pass must reason about.
    one_way_count: usize,
}

/// A stand-in for L2: interrogate every registered mechanic and summarise the world's logic.
///
/// **This function is the experiment.** It is written once and never touched between the two runs
/// below. It holds `&dyn Mechanic` and so is structurally incapable of knowing whether it is talking
/// to Rust or to a VM.
fn analyze(
    graph: &NodeGraph,
    content: &ContentRegistry,
    mechanics: &MechanicRegistry,
    placed: &[(cv_core::Handle<cv_core::Node>, ObjectId)],
) -> Analysis {
    let rng = Rng::new(0xA11CE);
    let mut analysis = Analysis {
        profiles: Vec::new(),
        edges: Vec::new(),
        grants: Vec::new(),
        blocked: Vec::new(),
        laser_bounce: None,
        one_way_count: 0,
    };

    for id in mechanics.ids() {
        let m = mechanics.get(id);
        let ctx = Context::new(graph, content, placed, &rng, "analyze");

        analysis.profiles.push((
            m.label().to_string(),
            m.footprint(&ctx).map(|v| v.required_bounds().volume()),
            m.constraints(&ctx).len(),
        ));

        for t in m.affords(&ctx) {
            if !t.reversible {
                analysis.one_way_count += 1;
            }
            analysis
                .edges
                .push((t.kind, t.requires.clone(), t.reversible));
        }

        if let Some(cap) = m.grants(&ctx) {
            analysis.grants.push((id, cap));
        }

        if m.kind() == ContentKind::SurfaceProperty {
            let stops: Vec<FlowKind> = FlowKind::CORE
                .iter()
                .copied()
                .filter(|f| m.blocks(&ctx, *f))
                .collect();
            analysis.blocked.push((m.label().to_string(), stops));

            if let Some(dir) = m.redirects(&ctx, FlowKind::Laser, Vec3::new(0.0, 0.0, -1.0)) {
                analysis.laser_bounce = Some(dir);
            }
        }
    }
    analysis
}

fn fixture_world() -> (NodeGraph, ContentRegistry) {
    let mut g = NodeGraph::new(1.0, 42);
    let reach = g.add_child(g.root(), "reach").unwrap();
    let area = g.add_child(reach, "area").unwrap();
    let space = g.add_child(area, "space").unwrap();
    for h in [g.root(), reach, area, space] {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(20.0)))
            .unwrap();
        g.advance(h, NodeState::Realized).unwrap();
    }
    (g, ContentRegistry::new())
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[test]
fn swapping_rust_fixtures_for_vm_backed_mechanics_changes_nothing() {
    let ids = Ids::new();
    let (graph, content) = fixture_world();
    let placed = Vec::new();

    // The *same* pipeline function, over two implementations that share no code path.
    let from_rust = analyze(&graph, &content, &rust_mechanics(&ids), &placed);
    let from_vm = analyze(&graph, &content, &vm_mechanics(&ids), &placed);

    assert_eq!(
        from_rust, from_vm,
        "the pipeline must not be able to tell Rust fixtures from VM-backed mechanics"
    );

    // Sanity: the analysis is substantive, not two empty structs comparing equal.
    assert_eq!(from_rust.profiles.len(), 6);
    assert_eq!(from_rust.grants.len(), 1, "the key grants the dash");
    assert_eq!(from_rust.one_way_count, 1, "the ledge is a one-way commit");
    assert_eq!(from_rust.blocked.len(), 2, "glass and the mirror");
    assert!(from_rust.laser_bounce.is_some());
}

#[test]
fn the_analysis_is_deterministic() {
    let ids = Ids::new();
    let (graph, content) = fixture_world();
    let placed = Vec::new();
    let mechanics = rust_mechanics(&ids);
    let first = analyze(&graph, &content, &mechanics, &placed);
    let second = analyze(&graph, &content, &mechanics, &placed);
    assert_eq!(first, second);
}

#[test]
fn behaviour_survives_the_substitution_in_detail_not_just_in_aggregate() {
    let ids = Ids::new();
    let (graph, content) = fixture_world();
    let placed = Vec::new();
    let rng = Rng::new(1);

    let rust = rust_mechanics(&ids);
    let vm = vm_mechanics(&ids);

    for id in [
        ids.door, ids.ledge, ids.key, ids.dash, ids.glass, ids.mirror,
    ] {
        let ctx = Context::new(&graph, &content, &placed, &rng, "compare");
        let (a, b) = (rust.get(id), vm.get(id));
        assert_eq!(a.kind(), b.kind(), "kind differs for {id}");
        assert_eq!(a.label(), b.label(), "label differs for {id}");
        assert_eq!(
            a.footprint(&ctx),
            b.footprint(&ctx),
            "footprint differs for {id}"
        );
        assert_eq!(
            a.constraints(&ctx),
            b.constraints(&ctx),
            "constraints differ for {id}"
        );
        assert_eq!(a.affords(&ctx), b.affords(&ctx), "affords differs for {id}");
        assert_eq!(a.grants(&ctx), b.grants(&ctx), "grants differs for {id}");
        for flow in FlowKind::CORE {
            assert_eq!(
                a.blocks(&ctx, flow),
                b.blocks(&ctx, flow),
                "blocks({flow:?}) differs"
            );
            assert_eq!(
                a.redirects(&ctx, flow, Vec3::new(0.0, 0.0, -1.0)),
                b.redirects(&ctx, flow, Vec3::new(0.0, 0.0, -1.0)),
                "redirects({flow:?}) differs for {id}"
            );
        }
    }
}

#[test]
fn the_pipeline_needs_no_branch_for_content_without_behaviour() {
    // Unregistered content answers with the core's defaults, so `analyze` has no "does this have a
    // mechanic?" case — one fewer thing to get wrong at every call site.
    let (graph, content) = fixture_world();
    let placed = Vec::new();
    let empty = MechanicRegistry::new();
    let analysis = analyze(&graph, &content, &empty, &placed);
    assert!(analysis.profiles.is_empty());

    let ctx = Context::detached();
    let unknown = empty.get(ObjectId::derived("actor", "never_registered"));
    assert!(unknown.footprint(&ctx).is_none());
    assert!(unknown.affords(&ctx).is_empty());
}

#[test]
fn mechanics_see_the_world_through_context_only() {
    // A mechanic's answers may depend on committed world state; that arrives via Context, which is the
    // single argument every callback takes. There is no other channel to check.
    let ids = Ids::new();
    let (graph, content) = fixture_world();
    let space = graph.of_kind(NodeKind::Space).next().unwrap().0;
    let placed = vec![(space, ids.door), (space, ids.door)];
    let rng = Rng::new(3);

    let ctx = Context::new(&graph, &content, &placed, &rng, "x").at_scope(space);
    assert_eq!(ctx.count_placed(ids.door), 2);
    assert_eq!(ctx.count_placed_here(ids.door), 2);
    assert_eq!(ctx.count_realized(NodeKind::Space), 1);
    // Committed state only — nothing here is a forecast.
    assert_eq!(ctx.count_projected(NodeKind::Space), 0);
}

#[test]
fn requests_are_collected_but_carry_no_authority() {
    let ids = Ids::new();
    let (graph, content) = fixture_world();
    let placed = Vec::new();
    let rng = Rng::new(5);
    let mechanics = rust_mechanics(&ids);

    let mut ctx = Context::new(&graph, &content, &placed, &rng, "request");
    mechanics.get(ids.door).request(&mut ctx);
    let asked = ctx.take_requests();
    assert_eq!(asked.len(), 1, "the door asked for spacing");
    // `request` returned nothing, so the mechanic could not branch on the outcome. The pipeline is
    // free to ignore every one of these and remains correct.
    assert!(ctx.requests().is_empty());
}
