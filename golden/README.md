# Golden vectors

Byte-exact fixtures for the determinism guarantee (*same seed + same build ⇒ same output*; WASM is the
canonical cross-machine target). Tests read a fixture from `vectors/` and byte-compare it against
freshly computed output — any drift is a determinism regression.

## The fixtures

| File | Pins | Checked by |
|---|---|---|
| `m00_placeholder.bin` | the harness itself | `cv-determinism/tests/golden.rs` |
| `m01_root_seed_c0ffee.bin` | the root PRNG stream | `cv-determinism/tests/prng.rs` |
| `m01_fork_tree_seed42.bin` | labelled + indexed fork streams | `cv-determinism/tests/prng.rs` |
| `m02_determinism_probe.bin` | **the cross-target probe** — owned math, RNG, and geometry | `tests/cross_target.rs` **and** `scripts/wasm-golden.cjs` |

## Why `m02_determinism_probe.bin` is the important one

It is checked **twice, against two targets**:

1. `cargo test -p cv-determinism` computes the blob natively (x86-64) and compares it to the file.
2. `node scripts/wasm-golden.cjs` loads the same code compiled to **wasm32**, reads the blob out of
   linear memory, and compares it to the *same* file.

Either check alone proves nothing about portability. Both passing means native and WASM agree
byte-for-byte across every transcendental, every RNG stream, and the geometry kernels — which is the
actual cross-machine guarantee. Run both with `npm run verify:determinism`.

Values are stored as raw little-endian IEEE-754 bit patterns, never formatted text, so a one-ULP
difference cannot hide behind decimal rounding.

## Regenerating (blessing)

Set `CV_BLESS=1` and run the tests to rewrite fixtures from current output:

```
CV_BLESS=1 cargo test -p cv-determinism        # bash
$env:CV_BLESS='1'; cargo test -p cv-determinism  # PowerShell
```

Blessing is a **deliberate act**. A changed fixture in a diff means the numerical behaviour of the
engine changed, and it should be reviewed as carefully as any logic change — if it was not intended,
something regressed. Always re-run the wasm half afterwards, since blessing only regenerates from
native.
