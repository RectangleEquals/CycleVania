# Golden vectors

Byte-exact fixtures for the determinism guarantee (*same seed + same build ⇒ same output*; WASM is the
canonical cross-machine target). Tests read a fixture from `vectors/` and byte-compare against freshly
computed output — any drift is a determinism regression.

- **`vectors/`** — committed fixtures. `m00_placeholder.bin` is a stand-in exercising the harness
  (`crates/cv-determinism/tests/golden.rs`); it is replaced by real PRNG/math vectors in M01/M02.
- **Native vs. WASM** — from M02, the same inputs are run under both targets and both must match the
  same fixture (or a documented, enforced tolerance — bit-identical preferred).

Regenerating a fixture is a deliberate act (a determinism change), reviewed like any other diff.
