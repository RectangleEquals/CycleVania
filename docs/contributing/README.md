# Contributing to CycleVania

Working **on** the SDK itself. If you are building a game *with* it, you want [`../hosting/`](../hosting/)
and [`../scripting/`](../scripting/) instead.

| Page | What it covers |
|---|---|
| [`dev-workflow.md`](dev-workflow.md) | Toolchain pin, building native + wasm, the test/lint loop, IDE setup, and the determinism rules the engine enforces on its own source |
| `architecture.md` _(planned: M37)_ | Crate layout and why the boundaries fall where they do |
| `releasing.md` _(planned: M37)_ | Publishing the packages, the addon, and the WASM module |

## Before you change numerical behaviour

The engine's core promise is *same seed + same build ⇒ same output*, on native **and** wasm32. Two
mechanisms enforce it and will fail your build rather than let drift through — a `clippy.toml` ban on
the divergent operations, and byte-exact golden probes compared across both targets. Both are explained
in [`dev-workflow.md`](dev-workflow.md), and the fixtures themselves in
[`../../golden/README.md`](../../golden/README.md).

## See also

- [`../concepts/`](../concepts/) — the determinism contract and pipeline model, stated for readers
  rather than maintainers.
