# M00 · Scaffold & deterministic math core

**Goal**: a fresh, empty-but-green monorepo with the complete deterministic math foundation
(`Rng`, trig, noise, QEF, vectors, golden vectors) and the determinism guard test. Everything
later stands on this.

**Required reading**: [redesign 01 §Module layout](../redesign/01-architecture.md),
[redesign 02 (all)](../redesign/02-determinism.md).

**Prerequisites**: none (first milestone). Confirm the working tree is clean and the legacy state
is committed (`git status` shows nothing to lose) **before** deleting anything.

## Phase 0.1 — Clean slate

1. Delete the entire `packages/` directory. Do **not** touch `Docs/` (all of it stays, including
   prior-art folders), `.git/`, or root dotfiles.
2. Delete any legacy root config that conflicts with what Phase 0.2 creates (old
   `vitest.workspace.ts`, root `package.json`, `pnpm-workspace.yaml`, `tsconfig.base.json`) —
   they will be recreated fresh.

Verification: `git status` shows only deletions + the `Docs/final/` docs; nothing else.

## Phase 0.2 — Root scaffold

Create:

- `pnpm-workspace.yaml` — `packages: ["packages/*"]`.
- `package.json` (root, `"private": true`): scripts
  `"typecheck": "pnpm -r typecheck"`, `"test": "vitest run"`, `"test:watch": "vitest"`,
  `"inspector": "pnpm --filter @cyclevania/inspector dev"`. Dev deps: `typescript` (^5.6),
  `vitest` (^3), `@types/node`.
- `tsconfig.base.json`:

```jsonc
{
  "compilerOptions": {
    "target": "ES2022", "module": "ESNext", "moduleResolution": "Bundler",
    "strict": true, "noUncheckedIndexedAccess": true, "exactOptionalPropertyTypes": true,
    "noEmit": true, "skipLibCheck": true, "isolatedModules": true,
    "forceConsistentCasingInFileNames": true, "types": []
  }
}
```

- `vitest.config.ts` (root): `test.include: ["packages/*/src/**/*.test.ts"]`, default timeout
  15000 ms. No workspace file — one root config is the whole story (this is what makes
  root-only test runs the convention).
- `.gitignore` additions if missing: `node_modules/`, `dist/`, `*.local`.

Create `packages/core/`: `package.json` (`"name": "@cyclevania/core"`, `"private": true`,
`"type": "module"`, `"main": "./src/index.ts"`, **no dependencies**, script
`"typecheck": "tsc -p tsconfig.json"`), `tsconfig.json` extending the base with
`"include": ["src"]`. Create `packages/core/src/index.ts` exporting nothing yet
(`export {};` placeholder, replaced in Phase 0.4).

Verification: `pnpm install` succeeds; `pnpm -r typecheck` green; `pnpm test` reports no test
files (exit 0 with `--passWithNoTests` — add that flag to the root test script).

## Phase 0.3 — Port the math module

Create `packages/core/src/math/` by **porting from legacy git history** (spec-checking each file
against [redesign 02](../redesign/02-determinism.md) as you go). Legacy paths (retrieve with
`git show <legacy-commit>:packages/core/src/math/<file>`):

| New file | Legacy source | Notes |
|---|---|---|
| `rng.ts` | `math/rng.ts` | sfc32 + FNV-1a + splitmix32; `fork(label)` non-advancing. Ensure the full API from redesign 02 exists: `next`, `int`, `range`, `chance`, `pick`, `weighted`, `shuffle`, `triangular`, `fork`. Add any missing method (e.g. `weighted`, `triangular`) using only existing draws. |
| `trig.ts` | `math/trig.ts` | `dsin`, `dcos`, `datan`, `datan2`, `reduce`, `yawFromDirection`, `yawBasis`. Do not alter constants. |
| `vec.ts` | `math/vec.ts` | `Vec3` = `[number, number, number]`; add/sub/scale/mul/dot/cross/length/normalize/lerp3/vecEq/ZERO3. |
| `geom.ts` | `math/geom.ts` | `WorldBox`, clamp/clamp01/lerp/invLerp/smoothstep, box helpers (fromCenterHalf, center, size, overlap, containsPoint, union, unionAll). |
| `curve.ts` | `math/curve.ts` | easing/curve helpers. |
| `noise.ts` | `math/noise.ts` | seeded gradient noise + `fbm3` per redesign 02 (permutation from an `Rng` fork; 12 fixed gradients; smootherstep). |
| `qef.ts` | `math/qef.ts` | 3×3 least-squares; fixed-order accumulation; singular → mass point; clamp to cell. |
| `fnv1a` | inside legacy `rng.ts` | export it — fingerprints and kit hashing use it. |
| `golden/rng.golden.json`, `golden/trig.golden.json` | same paths | **byte-identical copies** — do not regenerate. |
| `golden/record.ts` + `scripts/gen-golden.ts` | same paths | the regeneration script, for deliberate reference changes only. |

Write `math/index.ts` barrel; re-export the whole surface from `core/src/index.ts` (replacing the
placeholder), matching the names in redesign 02.

## Phase 0.4 — Math tests

Create:

- `math/rng.test.ts` — fork is non-advancing (fork, then parent draws equal a no-fork control
  run); same label ⇒ identical child sequences; different labels ⇒ different; `weighted` respects
  weights over 10k draws (loose bounds); `shuffle` is a permutation and deterministic;
  `triangular` stays in bounds and centers (loose).
- `math/trig.test.ts` — `|dsin(x) − Math.sin(x)| ≤ 1e-7` over a sweep of ±4π (same for dcos);
  `datan2` quadrant correctness at the 8 compass points.
- `math/golden.test.ts` — replays the recorded draw scripts from `golden/*.json` and asserts
  byte-for-byte equality (port the legacy test).
- `math/noise.test.ts` — output within [−1, 1]; same seed ⇒ identical field samples; different
  seed ⇒ different; `fbm3` finite over a coarse 3D sweep.
- `math/qef.test.ts` — exact solution for a synthetic 3-plane intersection; mass-point fallback
  for degenerate (parallel-planes) input; result always inside the given cell bounds.

## Phase 0.5 — The diagnostics channel

Create `core/src/diagnostics.ts` per
[redesign 12 §Diagnostics & logging](../redesign/12-orchestration-and-host-integration.md):
`DiagLevel`, `DiagEvent`, `DiagnosticsSink`, `DiagnosticsConfig` exactly as specified, plus:

```ts
export const SILENT_SINK: DiagnosticsSink;                       // the default
export class MemorySink implements DiagnosticsSink { events: DiagEvent[]; }   // for tests + tooling
export class Diag {                                              // the emitter threaded through generation
  constructor(cfg: DiagnosticsConfig);
  error(code: string, message: string, path?: string, details?: Record<string, unknown>): void;
  warn(…): void; info(…): void; debug(…): void; trace(…): void;
  child(pathSegment: string): Diag;                              // appends to the path prefix
}
```

Rules (binding on every later milestone): the emitter filters by level, **guards the sink**
(a throwing sink is caught and disabled for the run — never propagates into generation), and is
**write-only** — no generation code may ever read from a sink or branch on the configured level
(beyond the emitter's own filtering). Every later milestone threads a `Diag` through its
composers (`child()` per Reach/Area/Space so `path` is always populated), emits `warn` for every
recorded relaxation (same stable code string as the meta entry), `error` before every `GenError`
throw, `info` at phase transitions, and `debug`/`trace` for choice narration (scheduler rolls,
layout iterations). Test now: level filtering, sink guarding, `child` path composition, and the
purity pattern (a function producing output while emitting trace events returns identical output
under `SILENT_SINK` and a `MemorySink` — the full-pipeline version of this test lands in M08).

## Phase 0.6 — The determinism guard

`packages/core/src/determinism-guard.test.ts`: recursively read every `.ts` file under
`packages/core/src` and `packages/examples/src` (skip `*.test.ts` and `math/trig.ts`'s reference
comments), asserting no match for `/Math\.random/` or `/Math\.(sin|cos|tan|atan)\b/`. Use
`node:fs` inside the test only (tests may use Node; `src` code may not — assert that too by
failing on `from "node:` or `require(` in non-test core files).

## Definition of Done

- [ ] `packages/` contains exactly `core/` with `src/math`, `src/diagnostics.ts`,
      `src/index.ts`, the guard test.
- [ ] Diagnostics tests green (filtering, sink guarding, path composition, purity pattern).
- [ ] `pnpm install`, `pnpm -r typecheck`, `pnpm test` all green from the repo root.
- [ ] Golden tests pass against the **copied** (not regenerated) JSON vectors.
- [ ] Guard test passes and demonstrably fails when a `Math.random()` is temporarily inserted
      (try it, revert it).
- [ ] No runtime dependencies in `core/package.json`.
- [ ] STOP — hand off for review. Do not commit.

## Pitfalls

- Do not "modernize" the RNG or trig internals — golden vectors pin them; a one-bit drift
  invisibly desyncs everything downstream.
- `noUncheckedIndexedAccess` will flag legacy index patterns during the port — fix with
  destructuring/`for-of`, never with `!`.
- Keep `exactOptionalPropertyTypes` in mind from the start: write `field?: T` consistently and
  never assign `undefined` explicitly.
