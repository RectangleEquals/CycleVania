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
//! the event stream, and the mechanic-interface values (traversals, volumes, per-flow answers) taken
//! from the fixtures rather than written by hand. The descriptor especially matters here — it is the
//! artifact that actually *ships* to a host, so it differing between targets would be a shipped bug
//! rather than an internal one.
//!
//! `scripts/wasm-golden.cjs` compares the wasm32 output of `examples/core_probe.rs` against the same
//! fixture `tests/cross_target.rs` checks natively.

use crate::arena::{Arena, Handle};
use crate::content::{ContentKind, ContentRegistry};
use crate::context::Context;
use crate::descriptor::{
    DescriptorBuilder, InstanceRecord, MeshRecord, Placement, PlacementReason, Rationale, ScopeRef,
    Socket, WorldDescriptor,
};
use crate::events::GenEvent;
use crate::fingerprint::{Fingerprint, FingerprintBuilder, ReproductionBundle};
use crate::fixtures::{Deflective, Door, Glass, KeyItem, Ledge, MovementCapability};
use crate::mechanic::{FlowKind, Mechanic, Traversal, TraversalKind, Volume};
use crate::node::{NodeGraph, NodeState};
use crate::object::{IdAllocator, ObjectHeader, ObjectId};
use crate::schedule::{
    AdaptiveRange, Curve, Progression, Schedule, ScheduleBook, SeedPolicy, Span, TargetOutcome,
};
use crate::serialize::{to_bytes, Deserialize, Reader, SerResult, Serialize, Writer};
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
    traversals: Vec<Traversal>,
    volumes: Vec<Volume>,
    flows: Vec<FlowKind>,
    schedules: ScheduleBook,
    seed_policy: SeedPolicy,
    schedule_math: Vec<u8>,
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
        w.write(&self.traversals);
        w.write(&self.volumes);
        w.write(&self.flows);
        w.write(&self.schedules);
        w.write(&self.seed_policy);
        w.bytes(&self.schedule_math);
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
        (ContentKind::Capability, "blink_dash", 0x1003),
        (ContentKind::Biome, "caverns", 0x1004),
        (ContentKind::Motif, "ruins", 0x1005),
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
                weight: f64::from(i) + 0.5,
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
        ObjectId::derived("capability", "blink_dash"),
        ObjectId::derived("actor", "crawler/door_heavy").child("mesh"),
        ObjectId::derived("", ""),
        ObjectId::NONE,
    ];

    let (registry, bundle) = build_registry_and_bundle();
    let scopes = build_scopes();
    let descriptor = build_descriptor(&scopes, bundle.fingerprint, bundle.seed);
    let events = build_events(bundle.fingerprint, bundle.seed);
    let (traversals, volumes, flows) = build_mechanic_values();
    let (schedules, seed_policy, schedule_math) = build_schedule_values();
    ProbeWorld {
        ids,
        nodes,
        derived,
        scopes,
        registry,
        bundle,
        descriptor,
        events,
        traversals,
        volumes,
        flows,
        schedules,
        seed_policy,
        schedule_math,
    }
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
    b.finish()
}

/// The mechanic-interface value types, exercised through the fixtures rather than by hand — so the
/// probe covers what a mechanic actually *returns*, not just that the types encode.
fn build_mechanic_values() -> (Vec<Traversal>, Vec<Volume>, Vec<FlowKind>) {
    let ctx = Context::detached();
    let dash = ObjectId::derived("capability", "blink_dash");

    let mut traversals = Vec::new();
    traversals.extend(Door::locked_by(dash).affords(&ctx));
    traversals.extend(Ledge.affords(&ctx));
    traversals.extend(MovementCapability::new("Blink Dash", TraversalKind::Blink).affords(&ctx));
    traversals.push(Traversal::gated(TraversalKind::Custom(9), [dash]).one_way());

    let volumes: Vec<Volume> = [
        Door::locked_by(dash).footprint(&ctx),
        KeyItem::granting(dash).footprint(&ctx),
        Some(Volume::with_clearance(
            Aabb::new(Vec3::new(-0.1, 0.0, 3.3), Vec3::new(1.9, 4.25, 4.0)),
            0.125,
        )),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Which flows each surface stops — the per-flow answers, not just the enum tags.
    let mut flows = Vec::new();
    for f in FlowKind::CORE {
        if Glass.blocks(&ctx, f) {
            flows.push(f);
        }
        if Deflective::facing(Vec3::Z).blocks(&ctx, f) {
            flows.push(f);
        }
    }
    flows.push(FlowKind::Custom(77));

    (traversals, volumes, flows)
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
    book.set(ObjectId::derived("biome", "caverns"), Schedule::always());

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
