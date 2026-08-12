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
//! graph across every kind and lifecycle state, the content registry, and a fingerprint computed from
//! it. `scripts/wasm-golden.cjs` compares the wasm32 output of `examples/core_probe.rs` against the
//! same fixture `tests/cross_target.rs` checks natively.

use crate::arena::{Arena, Handle};
use crate::content::{ContentKind, ContentRegistry};
use crate::fingerprint::{FingerprintBuilder, ReproductionBundle};
use crate::node::{NodeGraph, NodeState};
use crate::object::{IdAllocator, ObjectHeader, ObjectId};
use crate::serialize::{to_bytes, Deserialize, Reader, SerResult, Serialize, Writer};
use cv_determinism::{Aabb, Vec3};

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
}

impl Serialize for ProbeWorld {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.ids);
        w.write(&self.nodes);
        w.write(&self.derived);
        w.write(&self.scopes);
        w.write(&self.registry);
        w.write(&self.bundle);
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
    ProbeWorld {
        ids,
        nodes,
        derived,
        scopes: build_scopes(),
        registry,
        bundle,
    }
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
