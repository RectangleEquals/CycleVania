//! Golden-vector harness (stub).
//!
//! M00 proves the read + byte-compare loop works against a committed fixture under `golden/vectors/`.
//! M01/M02 replace the placeholder with real PRNG/math vectors and add native↔WASM parity fixtures.

use std::path::PathBuf;

/// Resolve a fixture under the workspace-root `golden/vectors/` dir.
fn golden_path(name: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/cv-determinism; the golden dir is two levels up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/vectors")
        .join(name)
}

/// Byte-compare `actual` against the committed golden fixture `name`.
fn assert_golden_bytes(name: &str, actual: &[u8]) {
    let path = golden_path(name);
    let expected = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("missing golden fixture {}: {e}", path.display()));
    assert_eq!(expected, actual, "golden mismatch for {name}");
}

#[test]
fn harness_reads_and_compares() {
    // Placeholder vector: the ASCII bytes "cyclevania-m00". Real vectors arrive in M01.
    assert_golden_bytes("m00_placeholder.bin", b"cyclevania-m00");
}
