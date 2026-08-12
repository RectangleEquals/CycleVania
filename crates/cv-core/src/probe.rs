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
//! The blob is a serialized object graph: arenas with vacant slots, cyclic handle references, ids,
//! strings, and floats. `scripts/wasm-golden.cjs` compares the wasm32 output of
//! `examples/wasm_probe.rs` against the same fixture `tests/cross_target.rs` checks natively.

use crate::arena::{Arena, Handle};
use crate::object::{IdAllocator, ObjectHeader, ObjectId};
use crate::serialize::{to_bytes, Deserialize, Reader, SerResult, Serialize, Writer};

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

/// The probe world: an allocator plus an arena containing holes and cycles.
struct ProbeWorld {
    ids: IdAllocator,
    nodes: Arena<ProbeNode>,
    derived: Vec<ObjectId>,
}

impl Serialize for ProbeWorld {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.ids);
        w.write(&self.nodes);
        w.write(&self.derived);
    }
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

    ProbeWorld {
        ids,
        nodes,
        derived,
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
