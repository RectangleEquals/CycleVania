//! The native half of the cross-target determinism guarantee (M02).
//!
//! This pins the canonical probe blob to a committed fixture. The **wasm32** half lives in
//! `scripts/wasm-golden.cjs`, which loads `examples/wasm_probe.rs` compiled to wasm, reads the blob
//! out of linear memory, and compares it against *this same file*. One fixture, two targets — that
//! agreement is the guarantee, and neither half proves it alone.
//!
//! Run both: `npm run verify:determinism`.

mod common;
use common::assert_golden_bytes;
use cv_determinism::probe::determinism_probe;

/// The canonical fixture both targets are checked against.
///
/// Deliberately unprefixed by milestone: this is a *living* artifact that grows as the crate does
/// (M02 added the math and geometry; later milestones will extend it), so tying its name to one
/// milestone would go stale immediately.
pub const FIXTURE: &str = "determinism_probe.bin";

#[test]
fn probe_matches_the_golden_fixture() {
    assert_golden_bytes(FIXTURE, &determinism_probe());
}

#[test]
fn probe_is_reproducible_across_calls() {
    // A weaker check than the fixture, but it isolates "unstable within a process" (hash iteration
    // order, uninitialised memory) from "differs across builds".
    let a = determinism_probe();
    for _ in 0..4 {
        assert_eq!(a, determinism_probe());
    }
}

#[test]
fn probe_covers_every_subsystem() {
    // Guards against the blob silently shrinking to a stub and the fixture being re-blessed to match.
    let blob = determinism_probe();
    assert!(
        blob.len() >= 3000,
        "probe blob shrank to {} bytes — did a section get dropped?",
        blob.len()
    );
    assert_eq!(
        blob.len() % 4,
        0,
        "probe blob should be whole 4/8-byte values"
    );
    // Not all zeros, not all one repeated byte.
    assert!(
        blob.iter().any(|&b| b != blob[0]),
        "probe blob looks degenerate"
    );
}
