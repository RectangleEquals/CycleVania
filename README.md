# CycleVania

An SDK for **deterministic procedural generation of 3D metroidvania levels** — a Rust core
compiled to a native Node addon **and** WASM, a custom generation-scripting language (**CVScript**), a
host-facing TypeScript surface, and a web editor.

> **Status: pre-alpha.** Bootstrapping (milestone **M00**). Almost nothing works yet — this is the
> workspace skeleton. The build order is tracked privately.

## What it is

- **Deterministic by construction** — *same seed + same build ⇒ same output*; WASM is the canonical
  cross-machine target. One owned PRNG, owned math, no clock, no ambient randomness.
- **A generation pipeline (L0–L6)** — content → schedules → mission graph → skeleton → volume → geometry
  → finalize, emitting a `WorldDescriptor` of **structural data** the host consumes however it likes.
- **CVScript** — a small generation-only language compiled to an owned bytecode VM embedded in the core;
  it runs at the host's *generation* time (which may be runtime), never the gameplay simulation.

## Workspace layout

```
crates/
  cv-determinism/   owned PRNG, owned math, ordered containers
  cv-core/          data model, L0–L6 pipeline, solver, scheduling, Context API
  cv-script/        CVScript lexer, parser, analyzer, checkers, bytecode compiler
  cv-vm/            the owned bytecode VM + api-dispatch (embedded by cv-core)
  cv-bindings/      native Node addon (napi-rs v3) + WASM module (wasm-bindgen)
  cv-cli/           the `cv` headless CLI
  cv-editor-backend/ WebSocket service wrapping cv-core for the browser editor
golden/             byte-exact determinism fixtures
docs/               the manual (concepts / scripting / hosting / editor / contributing)
```

## Quick start

```
cargo build && cargo test
cargo run -p cv-cli -- --version
```

Start at [`docs/README.md`](docs/README.md), which routes by what you are trying to do. For WASM builds,
the Node addon round-trip, and editor setup, see
[`docs/contributing/dev-workflow.md`](docs/contributing/dev-workflow.md).
