# CycleVania

An SDK for **deterministic procedural generation of 3D metroidvania levels** — a Rust core compiled to a
native Node addon **and** WASM, a visual authoring system (**CVScript**), a host-facing TypeScript
surface, and a web editor.

> **Status: pre-alpha.** The core, the manifest-driven API surface and the visual toolchain exist; the
> spatial layers, the bindings surface and the editor do not yet. Nothing is stable. The build order is
> tracked privately.

## What it is

- **Deterministic by construction** — *same seed + same build ⇒ same output*; WASM is the canonical
  cross-machine target. One owned PRNG, owned math, no clock, no ambient randomness.
- **A generation pipeline (L0–L5)** — content → mission graph → skeleton → volume → geometry → finalize,
  emitting a `WorldDescriptor` of **structural data** the host consumes however it likes. There is no
  scheduling layer: schedules are declared on content and arbitrated inside the L1 solve, where they can
  backtrack.
- **CVScript** — a **visual** generation-only language. A developer authors *schematics* — node graphs
  and structured documents, not text — which compile to an owned bytecode VM embedded in the core. It
  runs at the host's *generation* time (which may be runtime), never the gameplay simulation.
- **Game-agnostic** — no fixed mechanic vocabulary. A host registers its own unlocks, items, triggers
  and puzzles; the core assumes no baseline, not even jumping.

## Workspace layout

```
crates/
  cv-determinism/   owned PRNG, owned math, ordered containers
  cv-manifest/      the tier-1 API manifest: the one hand-authored declaration of the core surface
  cv-api/           generated from the manifest — the tier-1 surface as a descriptor table
  cv-cvb/           CVB, the block notation: one parser and canonical writer for three formats
  cv-core/          data model, L0–L5 pipeline, solver, Context API
  cv-compile/       schematic graph → analysed → lowered → bytecode
  cv-assets/        .cvcurve / .cvunlock, mesh import, the project descriptor
  cv-vm/            the owned bytecode VM + api-dispatch (embedded by cv-core)
  cv-bindings/      native Node addon (napi-rs v3) + WASM module (wasm-bindgen)
  cv-cli/           the `cv` headless CLI
  xtask/            build tooling — regenerates every artifact derived from the manifest
golden/             byte-exact determinism fixtures
manifest/           tier1.toml — edit this, never the generated output
docs/               the manual (concepts / authoring / hosting / editor / contributing)
```

**The editor is not in this list, and that is deliberate.** It is a self-contained TypeScript project
that consumes the bindings like any other host — so a shipped game never carries editor code, because
the editor was never in the library.

## Quick start

```
cargo build && cargo test
cargo run -p cv-cli -- --version
cargo xtask check                 # fail if any generated artifact is stale
```

Start at [`docs/README.md`](docs/README.md), which routes by what you are trying to do. For WASM builds,
the Node addon round-trip, and the determinism rules the engine enforces on itself, see
[`docs/contributing/dev-workflow.md`](docs/contributing/dev-workflow.md).
