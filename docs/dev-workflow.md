# Dev workflow

The **CLI is authoritative**; IDEs are conveniences. The project never depends on a single IDE's Rust
support being current (Design/v0.1 toolchain decision).

## Toolchain

Pinned in `rust-toolchain.toml` (`1.89.0` + `rust-analyzer`, `rustfmt`, `clippy`, and the
`wasm32-unknown-unknown` target). `rustup` auto-installs the pin on first use; or explicitly:

```
rustup toolchain install 1.89.0 --profile minimal \
  --component rust-analyzer --component rustfmt --component clippy \
  --target wasm32-unknown-unknown
```

> **Why `rust-analyzer` is in the components list.** Because the toolchain is *pinned*, VS Code's
> bundled rust-analyzer refuses to work against it ("a toolchain too old for the extension shipped
> rust-analyzer") and prompts you to add the component. Listing it here means the version matched to
> **this** toolchain is installed and used, so the pin and the IDE agree out of the box. Keep it in the
> file whenever the pinned channel changes.

## Build & test (CLI, source of truth)

```
cargo build                      # native workspace
cargo test                       # all crates (incl. the golden-vector harness)
cargo run -p cv-cli -- --version # the `cv` CLI
cargo run -p cv-cli -- build --dry

# WASM: the pure library crates + the wasm-featured bindings
cargo build -p cv-core -p cv-vm -p cv-script -p cv-determinism --target wasm32-unknown-unknown
cargo build -p cv-bindings --no-default-features --features wasm --target wasm32-unknown-unknown

# Native Node addon round-trip
npm run verify:addon             # cargo build -p cv-bindings && load it from Node

# Determinism: native golden vectors + the wasm32 cross-target check
npm run verify:determinism

# Everything
npm run verify
```

## The determinism rules (what will bite you)

The engine's core promise is *same seed + same build ⇒ same output*, on native **and** wasm. Two
mechanisms enforce it, and both will fail your build rather than let drift through:

1. **`clippy.toml` denies the divergent operations** workspace-wide — platform transcendentals
   (`f64::sin`, `powf`, `ln`, …), `mul_add` (FMA), and clock reads. Use `cv_determinism::math`
   instead; the error message names the replacement. Basic `+ - * /` and `sqrt` are *not* banned —
   IEEE-754 requires those to be correctly rounded, so they are already identical everywhere.
   Run `cargo clippy --all-targets` before committing.
2. **The cross-target golden probe** (`golden/vectors/m02_determinism_probe.bin`) is byte-compared
   against both the native build and a wasm32 build. If you change numerical behaviour, that fixture
   changes — which is fine when intended, and a bug report when not. See `golden/README.md` for how
   to bless it.

New float-heavy code belongs in `cv-determinism`, exposed through `math` / `geom`, rather than
open-coded in the pipeline crates.

## Editors (either works; neither is required)

- **VS Code** — install *rust-analyzer*. Open the workspace root; it reads `rust-toolchain.toml`.
- **Rider** — enable the *Rust* plugin. Point it at the workspace `Cargo.toml`.

Format + lint before committing: `cargo fmt` and `cargo clippy --all-targets`.
