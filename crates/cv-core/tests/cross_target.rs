//! The native half of cv-core's cross-target determinism check (M03).
//!
//! Pins the serialized data-model probe to a committed fixture. The **wasm32** half lives in
//! `scripts/wasm-golden.cjs`, which runs `examples/core_probe.rs` compiled to wasm and compares
//! against *this same file*. Both matching is what proves the binary format is target-independent —
//! most importantly that no `usize` (64-bit native, **32-bit on wasm32**) reached the wire.
//!
//! Run both halves: `npm run verify:determinism`.

mod common;
use common::assert_golden_bytes;
use cv_core::probe::determinism_probe;

/// The canonical fixture both targets are checked against.
///
/// Unprefixed by milestone: a living artifact that grows with the crate (M03 added the arena,
/// identity and serialization; M04 the scope graph).
pub const FIXTURE: &str = "core_probe.bin";

#[test]
fn probe_matches_the_golden_fixture() {
    assert_golden_bytes(FIXTURE, &determinism_probe());
}

#[test]
fn probe_is_reproducible_across_calls() {
    // Isolates "unstable within a process" (hash iteration order, uninitialised memory) from
    // "differs across builds", which the fixture covers.
    let a = determinism_probe();
    for _ in 0..4 {
        assert_eq!(a, determinism_probe());
    }
}

#[test]
fn probe_carries_the_format_envelope() {
    let blob = determinism_probe();
    assert_eq!(
        &blob[..4],
        &cv_core::serialize::MAGIC,
        "probe must start with the magic bytes"
    );
    assert_eq!(
        u32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]),
        cv_core::serialize::FORMAT_VERSION,
    );
    // Guards against the blob quietly shrinking to a stub that a re-bless would then enshrine.
    assert!(
        blob.len() >= 500,
        "probe blob shrank to {} bytes",
        blob.len()
    );
}
