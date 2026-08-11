# Dev workflow

The **CLI is authoritative**; IDEs are conveniences. The project never depends on a single IDE's Rust
support being current (Design/v0.1 toolchain decision).

## Toolchain

Pinned in `rust-toolchain.toml` (`1.89.0` + `rustfmt`, `clippy`, and the `wasm32-unknown-unknown` target).
`rustup` auto-installs the pin on first use; or explicitly:

```
rustup toolchain install 1.89.0 --profile minimal --component rustfmt clippy --target wasm32-unknown-unknown
```

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
```

## Editors (either works; neither is required)

- **VS Code** — install *rust-analyzer*. Open the workspace root; it reads `rust-toolchain.toml`.
- **Rider** — enable the *Rust* plugin. Point it at the workspace `Cargo.toml`.

Format + lint before committing: `cargo fmt` and `cargo clippy --all-targets`.
