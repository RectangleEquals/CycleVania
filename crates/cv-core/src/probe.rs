//! cv-core's **cross-target determinism probe** — the data model's counterpart to
//! [`cv_determinism::probe`].
//!
//! # What this is actually guarding against
//!
//! The serialization format has one genuinely target-sensitive hazard: **`usize` is 64-bit on native
//! and 32-bit on wasm32**. Writing one directly would emit different bytes on the two targets while
//! every test on a single target still passed. [`Writer`](crate::Writer) prevents that structurally by
//! offering no `usize` method — but "the API makes it hard" is a weaker claim than "both targets
//! produce these exact bytes", so this probe makes the strong claim checkable.
//!
//! Hashing adds a second hazard on top of that. Content-addressed ids, `BTreeMap` ordering and the
//! fingerprint digest all have to land on identical bytes across targets, and none of that follows
//! from the serializer being portable — so the registry and a reproduction bundle are in here too.
//!
//! The blob covers: arenas with vacant slots and cyclic handle references, object identity, the scope
//! graph across every kind and lifecycle state, the content registry, a fingerprint computed from it,
//! the host-facing `WorldDescriptor` (both placement forms, mirroring, sockets, tags, rationale), and
//! and the event stream, taken
//! from the fixtures rather than written by hand. The descriptor especially matters here — it is the
//! artifact that actually *ships* to a host, so it differing between targets would be a shipped bug
//! rather than an internal one.
//!
//! A third class sits alongside those: **arithmetic that decides structure**. `AdaptiveRange`'s target
//! formula, the spine's coverage and surplus distribution, and M11's spatial primitives do not merely
//! encode differently if they drift — they put the boss room somewhere else, or the laser on the wrong
//! side of the glass. Those are swept across their input ranges here, not sampled at one point; the
//! ray sweep deliberately includes the origins that sit *exactly* on a slab boundary, which is where
//! the naive slab test produces NaN.
//!
//! `scripts/wasm-golden.cjs` compares the wasm32 output of `examples/core_probe.rs` against the same
//! fixture `tests/cross_target.rs` checks natively.

use crate::arena::{Arena, Handle};
use crate::content::{ContentKind, ContentRegistry};
use crate::descriptor::{
    DescriptorBuilder, InstanceRecord, MeshRecord, Placement, PlacementReason, Rationale, ScopeRef,
    Socket, SpineSlotTag, WorldDescriptor,
};
use crate::events::GenEvent;
use crate::fingerprint::{Fingerprint, FingerprintBuilder, ReproductionBundle};
use crate::geometry::{CoarseGeometry, Collider, Face, Hit};
use crate::mission::{MissionEdge, Rule};
use crate::node::{NodeGraph, NodeKind, NodeState};
use crate::object::{IdAllocator, ObjectHeader, ObjectId};
use crate::schedule::{
    AdaptiveRange, Curve, Progression, Schedule, ScheduleBook, SeedPolicy, Span, TargetOutcome,
};
use crate::serialize::{to_bytes, Deserialize, Reader, SerResult, Serialize, Writer};
use crate::spine::{
    Coverage, SlotRole, SpineSegment, SpineSlot, SpineTemplate, Strictness, UnlockRef,
};
use cv_determinism::{Aabb, Mat4, Quat, Rng, Transform, Vec3};

/// A probe node: carries every primitive the format supports, plus handles that form cycles.
#[derive(Clone, Debug, PartialEq)]
struct ProbeNode {
    header: ObjectHeader,
    links: Vec<Handle<ProbeNode>>,
    parent: Option<Handle<ProbeNode>>,
    weight: f64,
    count: u64,
    delta: i64,
    enabled: bool,
}

impl Serialize for ProbeNode {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.header);
        w.write(&self.links);
        w.write(&self.parent);
        w.f64(self.weight);
        w.u64(self.count);
        w.i64(self.delta);
        w.bool(self.enabled);
    }
}

impl Deserialize for ProbeNode {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(ProbeNode {
            header: r.read()?,
            links: r.read()?,
            parent: r.read()?,
            weight: r.f64()?,
            count: r.u64()?,
            delta: r.i64()?,
            enabled: r.bool()?,
        })
    }
}

/// The probe world: an allocator plus an arena containing holes and cycles, a scope graph, and the
/// content registry with the fingerprint derived from it.
struct ProbeWorld {
    ids: IdAllocator,
    nodes: Arena<ProbeNode>,
    derived: Vec<ObjectId>,
    scopes: NodeGraph,
    registry: ContentRegistry,
    bundle: ReproductionBundle,
    descriptor: WorldDescriptor,
    events: Vec<GenEvent>,
    schedules: ScheduleBook,
    seed_policy: SeedPolicy,
    schedule_math: Vec<u8>,
    rules: Vec<Rule>,
    mission_edges: Vec<MissionEdge>,
    strictness: Vec<Strictness>,
    unlock_refs: Vec<UnlockRef>,
    spine_math: Vec<u8>,
    faces: Vec<Face>,
    hits: Vec<Hit>,
    geometry_math: Vec<u8>,
}

impl Serialize for ProbeWorld {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.ids);
        w.write(&self.nodes);
        w.write(&self.derived);
        w.write(&self.scopes);
        w.write(&self.registry);
        w.write(&self.bundle);
        w.write(&self.descriptor);
        w.write(&self.events);
        w.write(&self.schedules);
        w.write(&self.seed_policy);
        w.bytes(&self.schedule_math);
        w.write(&self.rules);
        w.write(&self.mission_edges);
        w.write(&self.strictness);
        w.write(&self.unlock_refs);
        w.bytes(&self.spine_math);
        w.write(&self.faces);
        w.write(&self.hits);
        w.bytes(&self.geometry_math);
    }
}

/// A registry plus the recipe fingerprint computed from it.
///
/// Included in the probe because fingerprinting layers *hashing* on top of serialization — content-
/// addressed ids, `BTreeMap` ordering, and the content digest all have to land on the same bytes on
/// both targets, and none of that is implied by the serializer alone being portable.
fn build_registry_and_bundle() -> (ContentRegistry, ReproductionBundle) {
    let mut registry = ContentRegistry::new();
    for (kind, path, digest) in [
        (ContentKind::Actor, "crawler/door_heavy", 0x1001u64),
        (ContentKind::Item, "crawler/key_bronze", 0x1002),
        (ContentKind::UnlockTable, "unlocks/core", 0x1003),
        (ContentKind::Puzzle, "crawler/gate_relay", 0x1004),
        (ContentKind::Spine, "spines/ascent", 0x1005),
        (ContentKind::Component, "hinge", 0x1006),
        (ContentKind::SurfaceProperty, "portalable", 0x1007),
        (ContentKind::StaticMesh, "kit/door_a", 0x1008),
        (ContentKind::CurveTable, "curves/jump", 0x1009),
    ] {
        registry.register(kind, path, digest).unwrap();
    }

    let fingerprint = FingerprintBuilder::new("probe-0.1.0")
        .content(&registry)
        .script("door.cvs", 0xDEAD_BEEF)
        .script("key.cvs", 0x0BAD_F00D)
        .config_f64("worldScale", 1.5)
        .config_f64("awkward", 0.1) // not representable in binary
        .config_u64("reachTarget", 6)
        .config_i64("offset", -42)
        .config_bool("legibility", true)
        .config_str("preset", "crawler")
        .finish();

    let bundle =
        ReproductionBundle::new(fingerprint, 0xC0FF_EE00).with_output(0x1234_5678_9ABC_DEF0);
    (registry, bundle)
}

/// A scope graph spanning every kind, every lifecycle state, adjacency, and envelopes — so node
/// serialization is covered by the cross-target check too.
fn build_scopes() -> NodeGraph {
    let mut g = NodeGraph::new(1.5, 0x0BAD_C0DE);
    let world = g.root();
    g.set_envelope(world, Aabb::new(Vec3::splat(-500.0), Vec3::splat(500.0)))
        .unwrap();

    // A realized branch...
    let near = g.add_child(world, "reach_near").unwrap();
    let area = g.add_child(near, "area_entry").unwrap();
    let spaces: Vec<_> = (0..3)
        .map(|i| g.add_child(area, format!("space_{i}")).unwrap())
        .collect();
    let ledge = g.add_child(spaces[1], "ledge").unwrap();
    for w in spaces.windows(2) {
        g.connect(w[0], w[1]).unwrap();
    }
    g.connect(spaces[2], spaces[0]).unwrap(); // a loop

    for (i, h) in [world, near, area].iter().enumerate() {
        let s = 100.0 / (i as f64 + 1.0);
        g.set_envelope(*h, Aabb::new(Vec3::splat(-s), Vec3::splat(s)))
            .unwrap();
    }
    for (i, s) in spaces.iter().enumerate() {
        // Deliberately awkward values: negative, fractional, non-representable in binary.
        let lo = Vec3::new(i as f64 * 12.5, -0.1, 3.3);
        g.set_envelope(*s, Aabb::new(lo, lo + Vec3::new(10.0, 4.25, 0.7)))
            .unwrap();
    }
    g.set_envelope(
        ledge,
        Aabb::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(2.0, 3.0, 4.0)),
    )
    .unwrap();
    for h in [world, near, area] {
        g.advance(h, NodeState::Realized).unwrap();
    }
    g.advance(spaces[0], NodeState::Realized).unwrap();
    g.advance(spaces[1], NodeState::Reserved).unwrap(); // mid-lifecycle
    g.advance(ledge, NodeState::Reserved).unwrap();
    // spaces[2] stays Projected.

    // ...and a wholly projected branch, as lazy generation leaves it.
    let far = g.add_child(world, "reach_far").unwrap();
    let far_area = g.add_child(far, "area_deep").unwrap();
    g.add_child(far_area, "space_unknown").unwrap();

    debug_assert!(g.check_invariants().is_none());
    g
}

/// Build the canonical probe world. Deterministic and free of any target-dependent input.
fn build() -> ProbeWorld {
    let mut ids = IdAllocator::new();
    let mut nodes: Arena<ProbeNode> = Arena::new();

    let handles: Vec<Handle<ProbeNode>> = (0..12)
        .map(|i| {
            nodes.insert(ProbeNode {
                header: ObjectHeader::new(ids.allocate(), format!("node_{i}")),
                links: Vec::new(),
                parent: None,
                // Values chosen to exercise sign, magnitude, and non-representable decimals.
                weight: (i as f64) * 0.1 - 0.55,
                count: (i as u64).wrapping_mul(0x0123_4567_89AB_CDEF),
                delta: -(i as i64) * 1_000_003,
                enabled: i % 3 == 0,
            })
        })
        .collect();

    // Remove a few to leave vacant slots and a populated free list, then reinsert so reuse (and the
    // resulting generation bumps) is part of the serialized layout.
    nodes.remove(handles[4]);
    nodes.remove(handles[7]);
    nodes.remove(handles[1]);
    let reused: Vec<Handle<ProbeNode>> = (0..2)
        .map(|i| {
            nodes.insert(ProbeNode {
                header: ObjectHeader::derived("probe", format!("reused_{i}")),
                links: vec![handles[0]],
                parent: Some(handles[2]),
                weight: i as f64 + 0.5,
                count: u64::MAX - i as u64,
                delta: i64::MIN + i as i64,
                enabled: i == 0,
            })
        })
        .collect();

    // Cycles: a ring through the survivors, plus a self-loop and cross-links.
    let ring = [handles[0], handles[2], handles[3], handles[5], handles[6]];
    for (i, h) in ring.iter().enumerate() {
        nodes[*h].links = vec![ring[(i + 1) % ring.len()], *h];
        nodes[*h].parent = Some(ring[(i + ring.len() - 1) % ring.len()]);
    }
    nodes[handles[8]].links = reused.clone();

    // Content-addressed ids — stable by construction, and must hash identically on both targets.
    let derived = vec![
        ObjectId::derived("actor", "crawler/door_heavy"),
        ObjectId::derived("item", "crawler/key_bronze"),
        ObjectId::derived("unlock", "blink_dash"),
        ObjectId::derived("actor", "crawler/door_heavy").child("mesh"),
        ObjectId::derived("", ""),
        ObjectId::NONE,
    ];

    let (registry, bundle) = build_registry_and_bundle();
    let scopes = build_scopes();
    let descriptor = build_descriptor(&scopes, bundle.fingerprint, bundle.seed);
    let events = build_events(bundle.fingerprint, bundle.seed);
    let (schedules, seed_policy, schedule_math) = build_schedule_values();
    let (rules, mission_edges) = build_mission_values();
    let (strictness, unlock_refs, spine_math) = build_spine_values();
    let (faces, hits, geometry_math) = build_geometry_values();
    ProbeWorld {
        ids,
        nodes,
        derived,
        scopes,
        registry,
        bundle,
        descriptor,
        events,
        schedules,
        seed_policy,
        schedule_math,
        rules,
        mission_edges,
        strictness,
        unlock_refs,
        spine_math,
        faces,
        hits,
        geometry_math,
    }
}

/// Spatial primitives across the cases where float behaviour actually differs.
///
/// These decide *where a beam stops* and *where a body ends up*, so a drift between targets would not
/// merely encode differently — it would put the laser on the wrong side of the glass. The sweep covers
/// the two hazards the module calls out (axis-parallel rays whose origin sits exactly on a slab
/// boundary, and distance ties between coincident boxes) plus the sweep/slide arithmetic, whose contact
/// distances are the ones a movement mechanic acts on.
fn build_geometry_values() -> (Vec<Face>, Vec<Hit>, Vec<u8>) {
    let mut geometry = CoarseGeometry::new();
    for i in 0..6 {
        let x = f64::from(i) * 2.5 - 3.75;
        geometry.add(
            Collider::new(
                ObjectId::derived("actor", &format!("box_{i}")),
                Aabb::new(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 1.5, 2.0, 2.0)),
            )
            .tagged_face(Face::NegX, ObjectId::derived("surface", "portalable")),
        );
    }
    // Two coincident boxes, so the tie-break is in the blob rather than merely asserted.
    geometry.add(Collider::new(
        ObjectId::derived("actor", "twin_a"),
        Aabb::new(Vec3::new(20.0, 0.0, 0.0), Vec3::new(21.0, 2.0, 2.0)),
    ));
    geometry.add(Collider::new(
        ObjectId::derived("actor", "twin_b"),
        Aabb::new(Vec3::new(20.0, 0.0, 0.0), Vec3::new(21.0, 2.0, 2.0)),
    ));

    let origins = [
        Vec3::new(-20.0, 1.0, 1.0),
        Vec3::new(-20.0, 0.0, 1.0), // exactly on the -Y boundary: the NaN case
        Vec3::new(-20.0, 2.0, 1.0), // exactly on the +Y boundary
        Vec3::new(-20.0, 0.5, 0.125), // an awkward, non-representable offset
        Vec3::new(0.25, 1.0, 1.0),  // starting inside a box
    ];
    let directions = [
        Vec3::X,
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(3.0, 0.0, 0.0), // un-normalized: must answer identically to the unit case
        Vec3::new(1.0, 0.25, 0.0),
        Vec3::new(1.0, -0.1, 0.05),
        Vec3::Y,
    ];

    let mut hits = Vec::new();
    let mut w = Writer::new();
    for origin in origins {
        for direction in directions {
            let all = geometry.raycast_all(origin, direction, 100.0);
            w.u32(all.len() as u32);
            for hit in &all {
                w.f64(hit.distance);
                w.write(&hit.point);
                w.write(&hit.collider);
                w.u8(u8::from(hit.from_inside));
            }
            if let Some(first) = all.first() {
                hits.push(*first);
            }
            w.bool(geometry.line_of_sight(origin, origin + direction * 40.0));
        }
    }

    // Sweep and slide: the contact distances a movement mechanic acts on.
    for half in [0.25f64, 0.5, 0.75] {
        for direction in [
            Vec3::X,
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(2.0, 1.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
        ] {
            let body = Aabb::from_center_extents(Vec3::new(-10.0, 1.0, 1.0), Vec3::splat(half));
            let swept = geometry.sweep(body, direction, 50.0);
            w.f64(swept.distance);
            w.write(&swept.end);
            let slid = geometry.slide_to_collision(body, direction, 50.0);
            w.f64(slid.distance);
            w.write(&slid.end);
        }
    }

    // Reflection about each face normal — the mirror rule, per axis.
    for face in Face::ALL {
        for incoming in [
            Vec3::new(1.0, -1.0, 0.0).normalized(),
            Vec3::new(0.3, 0.7, -0.2),
        ] {
            w.write(&CoarseGeometry::reflect(incoming, face.normal()));
        }
    }

    (Face::ALL.to_vec(), hits, w.finish())
}

/// Spine encodings and the arithmetic that decides structure.
///
/// A spine's promises are *structural* — which scope is the boss arena, which instances are covered.
/// If that arithmetic drifted between targets, two players on different platforms would get worlds
/// laid out differently from the same seed, which is exactly the failure this whole crate exists to
/// prevent. [`Coverage::Fraction`] is the sharp edge: it accumulates a curve, so it is float math
/// deciding topology.
fn build_spine_values() -> (Vec<Strictness>, Vec<UnlockRef>, Vec<u8>) {
    let strictness = vec![
        Strictness::Required,
        Strictness::Preferred,
        Strictness::Optional,
    ];
    let unlock_refs = vec![
        UnlockRef::Explicit(ObjectId::derived("unlock", "blink_dash")),
        UnlockRef::GrantedBy("precursor".into()),
        UnlockRef::Explicit(ObjectId::NONE),
    ];

    let mut w = Writer::new();
    // Adherence thresholds, and the keep/drop decision they drive across the whole dial.
    for tier in [
        Strictness::Required,
        Strictness::Preferred,
        Strictness::Optional,
    ] {
        w.f64(tier.adherence_threshold());
        let template = SpineTemplate::new(ObjectId::derived("spine", "probe"), NodeKind::Reach);
        for step in 0..=10 {
            let adherence = f64::from(step) / 10.0;
            let slot = SpineSlot::new("probe").strictness(tier);
            w.bool(template.clone().adherence(adherence).keeps(&slot));
        }
    }
    // Coverage patterns over several totals — the pattern-not-lottery guarantee, in bytes.
    let coverages = [
        Coverage::All,
        Coverage::Every(3),
        Coverage::Indices(vec![0, 2, 5, 99]),
        Coverage::Fraction(Curve::constant(0.5)),
        Coverage::Fraction(Curve::ramp(0.0, 1.0)),
        Coverage::Fraction(Curve::from_points([(0.0, 0.2), (0.5, 0.9), (1.0, 0.3)])),
    ];
    for coverage in &coverages {
        for total in [0usize, 1, 2, 7, 13] {
            let selected = coverage.selected(total);
            w.u32(selected.len() as u32);
            for i in selected {
                w.u32(i as u32);
            }
        }
    }
    // The budget floor: required slots plus the minimums of segments joining them.
    let crawl = SpineTemplate::new(ObjectId::derived("spine", "loop"), NodeKind::Reach)
        .slot(SpineSlot::new("start").role(SlotRole::Start))
        .slot(SpineSlot::new("capstone"))
        .slot(
            SpineSlot::new("terminal")
                .role(SlotRole::Goal)
                .adjacent_to("capstone"),
        )
        .segment(SpineSegment::new(
            "start",
            "capstone",
            AdaptiveRange::new(2, 5),
        ))
        .segment(SpineSegment::direct("capstone", "terminal"));
    w.u32(crawl.required_minimum());
    w.u32(crawl.kept_slots().len() as u32);
    // Surplus distribution — this decides how deep the capstone sits, so it is structure, not garnish.
    let kept = crawl.kept_slots();
    for capacity in [0usize, 3, 4, 5, 9, 17, 64] {
        for length in crawl.segment_lengths(&kept, capacity) {
            w.u32(length);
        }
        w.u32(u32::MAX); // separator, so differing lengths cannot alias
    }

    (strictness, unlock_refs, w.finish())
}

/// A descriptor covering both placement forms, mirroring, sockets, tags and rationale — the
/// host-facing output has to be byte-identical across targets too, since it is what ships.
fn build_descriptor(scopes: &NodeGraph, fingerprint: Fingerprint, seed: u64) -> WorldDescriptor {
    let mut b = DescriptorBuilder::new(scopes, fingerprint, seed);
    let space = ScopeRef(3);

    b.place(InstanceRecord {
        id: ObjectId::derived("instance", "door_1"),
        content: ObjectId::derived("actor", "crawler/door_heavy"),
        scope: space,
        placement: Placement::Trs(Transform::new(
            Vec3::new(1.5, -2.25, 3.125),
            Quat::from_axis_angle(Vec3::new(0.3, -0.6, 0.75), 1.234_567),
            Vec3::new(2.0, 0.5, 1.25),
        )),
        rationale: Rationale::detailed(PlacementReason::SolverRequired, "gate on edge 0→1"),
    });
    b.place(InstanceRecord {
        id: ObjectId::derived("instance", "key_1"),
        content: ObjectId::derived("item", "crawler/key_bronze"),
        scope: ScopeRef(4),
        // A mirrored placement: negative scale, still TRS.
        placement: Placement::Trs(Transform::from_scale(Vec3::new(-1.0, 1.0, 1.0))),
        rationale: Rationale::new(PlacementReason::Scheduled),
    });

    b.place_mesh(MeshRecord {
        id: ObjectId::derived("meshinst", "door_1_mesh"),
        mesh: ObjectId::derived("mesh", "kit/door_a"),
        scope: space,
        // A sheared placement, which can only be carried as a matrix.
        placement: Placement::from_matrix(
            Mat4::from(Transform::from_scale(Vec3::new(2.0, 1.0, 1.0)))
                * Mat4::from(Transform::from_rotation(Quat::from_axis_angle(
                    Vec3::Z,
                    0.7,
                ))),
        ),
        collision: vec![
            Aabb::new(Vec3::ZERO, Vec3::new(1.0, 0.2, 2.5)),
            Aabb::new(Vec3::new(-0.1, 0.0, 0.0), Vec3::new(0.9, 3.3, 0.7)),
        ],
        sockets: vec![
            Socket {
                name: "hinge".into(),
                transform: Transform::from_translation(Vec3::new(0.0, 0.0, 1.25)),
            },
            Socket {
                name: "latch".into(),
                transform: Transform::IDENTITY,
            },
        ],
        tags: vec![
            ObjectId::derived("surface", "portalable"),
            ObjectId::derived("surface", "deflective"),
        ],
        rationale: Rationale::new(PlacementReason::Connector),
    });

    // A spine tag, so the field a host reads a guarantee back through is in the blob too.
    let spaces: Vec<_> = scopes.of_kind(NodeKind::Space).map(|(h, _)| h).collect();
    b.tag_spine_slot(
        spaces[0],
        SpineSlotTag {
            template: ObjectId::derived("spine", "reach_loop"),
            slot: "capstone".into(),
        },
    );
    b.finish()
}

/// Scheduling config plus the **computed** results of the AdaptiveRange formula.
///
/// The config types matter because they are project data and therefore fingerprint inputs. The
/// computed values matter more: the formula does floating-point multiplication and a `floor`, so a
/// target landing on a boundary could in principle differ per target. Pinning the outputs — not just
/// the inputs — is what makes that checkable.
fn build_schedule_values() -> (ScheduleBook, SeedPolicy, Vec<u8>) {
    let mut book = ScheduleBook::new();
    book.set(
        ObjectId::derived("actor", "crawler/door_heavy"),
        Schedule::during(Span::new(0.2, 0.9)).weighted(Curve::from_points([
            (0.0, 0.1),
            (0.35, 0.75),
            (1.0, 0.9),
        ])),
    );
    book.set(
        ObjectId::derived("item", "crawler/key_bronze"),
        Schedule::during(Span::from(0.5)).weighted(Curve::ramp(0.05, 1.0)),
    );
    book.set(
        ObjectId::derived("puzzle", "crawler/gate_relay"),
        Schedule::always(),
    );

    let policy = SeedPolicy {
        lookahead: 3,
        lookbehind: 1,
        length: AdaptiveRange::new(4, 9)
            .with_repeat_tol(1.75)
            .with_jitter(2),
    };

    // Sweep the formula across all three regimes and awkward weights.
    let range = AdaptiveRange::new(3, 6).with_repeat_tol(1.5).with_jitter(2);
    let mut w = Writer::new();
    let mut rng = Rng::new(0x5C_1ED0_0001);
    for unique in [0u32, 1, 2, 3, 5, 10, 40] {
        for weight in [0.0, 0.1, 0.333_333_333_333_333_3, 0.5, 0.75, 1.0] {
            let r = range.resolve(unique, weight, &mut rng);
            w.u32(r.unique);
            w.u32(r.supported);
            w.u32(r.target);
            w.i32(r.jitter);
            w.f64(r.weight);
            w.f64(r.repeat_tol);
            w.u8(match r.outcome {
                TargetOutcome::Abundant => 0,
                TargetOutcome::Moderate => 1,
                TargetOutcome::Scarce => 2,
                TargetOutcome::Fixed => 3,
                TargetOutcome::Sampled => 4,
                TargetOutcome::Curved => 5,
            });
        }
    }
    // Curve evaluation at awkward fractions, where interpolation could drift.
    let curve = Curve::from_points([(0.0, 0.0), (0.3, 1.0), (0.55, 0.25), (1.0, 0.9)]);
    for i in 0..=20 {
        w.f64(curve.eval(Progression::new(i as f64 / 20.0)));
    }

    (book, policy, w.finish())
}

/// L2 rules and edges, including a nested combinator — the grammar is script-authored at M16, so its
/// encoding has to be stable across targets before anything writes one.
fn build_mission_values() -> (Vec<Rule>, Vec<MissionEdge>) {
    let dash = ObjectId::derived("unlock", "blink_dash");
    let grapple = ObjectId::derived("unlock", "grapple");
    let key = ObjectId::derived("unlock", "key_bronze");

    let rules = vec![
        Rule::Always,
        Rule::Never,
        Rule::has(dash),
        Rule::all_of([dash, key]),
        Rule::any_of([dash, grapple]),
        Rule::All(vec![
            Rule::has(key),
            Rule::Any(vec![
                Rule::has(dash),
                Rule::Not(Box::new(Rule::has(grapple))),
            ]),
        ]),
    ];

    // Edges need scope handles; the probe scope graph supplies real ones.
    let g = build_scopes();
    let spaces: Vec<_> = g.of_kind(NodeKind::Space).map(|(h, _)| h).collect();
    let edges = vec![
        MissionEdge::open(spaces[0], spaces[1]),
        MissionEdge::gated(spaces[1], spaces[2], Rule::has(dash)),
        MissionEdge::open(spaces[0], spaces[2]).one_way(),
        MissionEdge::gated(spaces[2], spaces[0], Rule::any_of([dash, grapple])).shortcut(),
    ];
    (rules, edges)
}

/// One of every event variant, in a fixed order.
fn build_events(fingerprint: Fingerprint, seed: u64) -> Vec<GenEvent> {
    vec![
        GenEvent::Started { fingerprint, seed },
        GenEvent::LayerProgress {
            layer: 2,
            fraction: 0.1,
        }, // not representable in binary
        GenEvent::ScopeAdvanced {
            scope: ScopeRef(3),
            state: NodeState::Realized,
        },
        GenEvent::Placed {
            instance: ObjectId::derived("instance", "door_1"),
            content: ObjectId::derived("actor", "crawler/door_heavy"),
            scope: ScopeRef(3),
        },
        GenEvent::Rejected {
            content: ObjectId::derived("actor", "statue"),
            scope: ScopeRef(4),
            reason: "footprint exceeds the space".into(),
        },
        GenEvent::Signal {
            name: "door_opened".into(),
            detail: "by key_bronze".into(),
        },
        GenEvent::Finished {
            instances: 2,
            meshes: 1,
        },
    ]
}

/// Compute the canonical cv-core determinism blob: the serialized probe world, envelope included.
pub fn determinism_probe() -> Vec<u8> {
    to_bytes(&build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::from_bytes;

    #[test]
    fn probe_is_stable_within_a_run() {
        assert_eq!(determinism_probe(), determinism_probe());
    }

    #[test]
    fn probe_is_substantial() {
        assert!(determinism_probe().len() > 500);
    }

    #[test]
    fn probe_world_round_trips() {
        // The arena inside must survive a round-trip, or the blob is not testing what it claims.
        let world = build();
        let bytes = to_bytes(&world.nodes);
        let back: Arena<ProbeNode> = from_bytes(&bytes).unwrap();
        assert_eq!(back, world.nodes);
    }
}
