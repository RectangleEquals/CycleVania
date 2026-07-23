# M13 · The CLI

**Goal**: `@cyclevania/cli` — headless `generate`, `validate`, `soak`, `report`, `diff`,
`export-diagram` over the public core API, emitting JSON/markdown/mermaid. The CLI is the CI
gate and the "no guesswork" report generator.

**Required reading**: [redesign 13 §The CLI](../redesign/13-inspector-and-tooling.md);
[redesign 10 §Serialization](../redesign/10-output-contract.md).

**Prerequisites**: M11 green (may be built before or after M12).

## Phase 13.1 — Scaffold

`packages/cli/`: `package.json` (`@cyclevania/cli`, deps: `@cyclevania/core`,
`@cyclevania/examples` — Node builtins only beyond that; hand-roll arg parsing, no commander),
`bin: { "cyclevania": "./src/main.ts" }` executed via `node --experimental-strip-types` or
`pnpm exec tsx` — pick **tsx as a devDependency** for reliability and document the invocation
(`pnpm cyclevania …` root script). `src/main.ts` dispatches subcommands; each subcommand is its
own module.

Global flags on every subcommand: `--log-level <error|warn|info|debug|trace>` (default `warn`)
routing the diagnostics channel to stderr as `[level] code path — message` lines (stable,
grep-able); long-running commands (`generate --geometry`, `soak`) render the `GenProgress`
stream as a single rewriting terminal progress line (plain appended lines when stderr is not a
TTY).

## Phase 13.2 — Bundles: `src/bundle.ts`

The **reproduction bundle** format (shared with M12's inspector — define it here, in a module the
inspector will import from core? No: bundles serialize *registry input*, which includes
callbacks). Resolution, contractual: a bundle references data by **module specifier + export
name** for callback-bearing registries (`{ registryModule: "@cyclevania/examples",
registryExport: "classicPreset" }`) plus inline JSON for pure-data overrides, dials, seed,
request log, and expected `registryFingerprint`/`generationVersion`. Loading = dynamic import +
`defineRegistry` + fingerprint check (mismatch ⇒ hard error explaining what changed, per
redesign 02's "what can legitimately change a world").

**Every `<bundle>` argument accepts a file path** — a bundle JSON file, a standalone dataset
file (a full registry or a single catalog/config slice, merged over `--base <preset>` when
partial), or, for `report`/`diff`, a previously generated world-descriptor JSON (consumed
read-only, no regeneration). Payload kind is sniffed by shape; ambiguity is a hard error naming
the missing discriminant. Import is export's symmetric peer: the round-trip test (export via
`generate -o` + bundle save → load → regenerate → byte-identical) is part of this milestone's
suite. Put the bundle types + load/save in
`core/src/descriptors/bundle.ts` (pure shapes + fingerprint check) with the dynamic-import
loader living in the CLI (core stays import-free).

## Phase 13.3 — Subcommands

| Command | Behavior | Test |
|---|---|---|
| `generate <bundle> [--seed S] [--reaches N] [--geometry] [-o out.json]` | realize N Reaches, write `stableStringify(worldDescriptor)` | golden: output byte-stable across runs |
| `validate <bundle>` | `defineRegistry` + template static checks; human-readable diagnostics; exit 1 on `GenError` | a broken fixture exits 1 and names the entry |
| `soak <bundle> --seeds N [--autosolve]` | solvability (+ optional autosolve) across N seeds; summary table (pass/fail, relaxation counts, timing); exit 1 on any failure | runs 25 seeds green on `classic` |
| `report <bundle> --seed S -o report.md` | the world report per redesign 13: mission-graph mermaid per Reach, sphere table, placement table, dial snapshot, schedule-vs-plan, relaxations, geometry stats when present | snapshot test: report contains the mermaid fence, sphere table headers, and the seed |
| `diff <bundleA> <bundleB>` | graph/placement/pacing diff (added/removed/moved placements; autosolve log length delta) | two seeds differ; same seed twice ⇒ "identical" |
| `export-diagram <bundle> --seed S --view mission [-o out.mmd]` | just the mermaid source | emitted mermaid parses (balanced brackets smoke check) |

`src/mermaid.ts` — mission-graph → mermaid generator **from descriptors only** (Regions colored
by role via class defs, edges labeled with rule summaries, sphere badges). M12's inspector
renders the same source — export it from the CLI package for reuse (`@cyclevania/cli/mermaid`)
or, better, place it in `core/src/descriptors/diagram.ts` (it's a pure descriptor→string
function; core-legal). **Choose core placement** so both tools share it without a cross-tool
dependency.

## Phase 13.4 — Tests (`packages/cli/src/cli.test.ts`)

Drive subcommands in-process (export a `runCli(argv): {code, stdout}` from `main.ts` — tests
never spawn shells). Cover the table above + `--help` output listing every subcommand.

## Definition of Done

- [ ] Every subcommand implemented with its test; `pnpm cyclevania soak` documented in the root
      README script.
- [ ] The mermaid generator lives in `core/src/descriptors/diagram.ts` and is descriptor-pure.
- [ ] Bundle fingerprint mismatch produces the explanatory error, not a stack trace.
- [ ] Full root suite + typecheck green. STOP — hand off. Do not commit.

## Pitfalls

- Windows paths in output files: use `node:path` and write with UTF-8 no-BOM; never hardcode
  separators.
- Keep `report` fast: it must run with `geometry` off unless `--geometry` is passed.
