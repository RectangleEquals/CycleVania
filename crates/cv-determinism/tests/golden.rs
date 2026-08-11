//! Golden-vector harness self-check. Proves the read + byte-compare loop works against a committed
//! fixture; the real determinism vectors live in `prng.rs` (M01) and grow through M02 (native↔WASM).

mod common;
use common::assert_golden_bytes;

#[test]
fn harness_reads_and_compares() {
    // Placeholder vector exercising the harness itself.
    assert_golden_bytes("m00_placeholder.bin", b"cyclevania-m00");
}
