# M10 · Orchestration

**Goal**: the `orchestration/` module — the async facade over the pure cores (progress,
cooperative yielding, cancellation), the `GenerationHorizon` prefetch helper, the worker seam,
and the finalized error taxonomy. Orchestrated output must be byte-identical to inline output.

**Required reading**: [redesign 12 §Sync cores… §Horizon… §Worker… §Error taxonomy](../redesign/12-orchestration-and-host-integration.md).

**Prerequisites**: M08 green (independent of M09).

**Porting references**: legacy `core/src/orchestration/{cancellation,orchestrator,horizon}.ts` —
cancellation + hook plumbing port directly; the horizon is redesigned around `ReachRequest`.

## Phase 10.1 — Errors: `core/src/errors.ts` (finalize)

Extend M01's `GenError` with subclasses used across the codebase (audit all existing `GenError`
throws and migrate): `RegistryError`, `TemplateError`, `RequestError`, `PlacementError`,
`BudgetError` — each carrying a structured `details` object (entry id, field, offenders) in
addition to the message. `GenCancelled` (facade-only). Document the taxonomy in the module
docstring per redesign 12's table.

## Phase 10.2 — Facade: `orchestration/async.ts`, `orchestration/cancellation.ts`, `orchestration/progress.ts`

`CancellationToken` (`cancel()`, `throwIfCancelled()`). `requestReachAsync(world, request,
hooks?)` per redesign 12's shapes — `OrchestrationHooks` now carries `onProgress?(p:
GenProgress)` and `diagnostics?: DiagnosticsConfig` (per-run override of the registry default).
Slicing, three granularities:

1. per phase, 2. per Area (`composeArea`, the M08 seam), and 3. **within the finish pass** —
   drive M07's `dualContourSteps` generator, yielding/checking between z-slabs so a large Area
   never blocks the frame budget.

Between every slice: `await hooks.shouldYield?.()`, `token?.throwIfCancelled()`, and a progress
report. `orchestration/progress.ts` implements `GenProgress` exactly per redesign 12: the static
per-phase **weight table** (defaults: template/selection/graph/fill 2 each, skeleton 8, volume
14, anchors 6, finish 60, assemble 4 — a dial), overall `fraction` monotonic non-decreasing
(assert internally), `phaseFraction` from slab/Area counts, `label` strings
("Meshing Area 3/5"), `elapsedMs` from a host-supplied `now?: () => number` hook (default
`Date.now` — **facade-only**; core never reads a clock). A cancelled run leaves the composer
**unchanged** (build into locals; commit to composer state only at the end — verify the request
log too).

## Phase 10.3 — Horizon: `orchestration/horizon.ts`

`GenerationHorizon` per redesign 12: constructor `(world, policy: HorizonPolicy, runner =
requestReachAsync)`; `noteAt(reachIndex)` (the player's position) triggers prefetch of up to
`policy.ahead` next slots via `policy.requestFor(nextIndex)`; caches descriptors; evicts realized
descriptors beyond `evictBehind` (they remain regenerable — eviction drops the cache entry only,
never the request log). Prefetched requests enter the log exactly like manual ones.

## Phase 10.4 — Worker seam: `orchestration/worker-adapter.ts`

`WorkerLike` interface (`post(job): Promise<result>`); `composeAreaJob` = plain-data in/out
wrapper around M08's `composeArea` (inputs: registry fingerprint check + serialized area inputs).
Ship an `inlineWorker` (same-thread) implementation; an actual Web Worker wrapper is
host/inspector territory. The contract test: job output via `inlineWorker` byte-equals direct
`composeArea`.

## Phase 10.5 — Tests (`orchestration/orchestration.test.ts`)

- **Parity**: `requestReachAsync` result byte-equals synchronous `requestReach` for the same
  `(seed, request)` (fresh composers), with and without geometry.
- **Cancellation**: cancel mid-generation (from an `onProgress` hook) ⇒ rejects `GenCancelled`;
  composer state and request log unchanged; a subsequent uncancelled request succeeds and
  byte-equals a never-cancelled control.
- **Progress**: overall `fraction` non-decreasing across the whole run, final = 1; finish-phase
  reports arrive per z-slab (count > areas on a multi-slab fixture); labels populated; with a
  fake `now`, `elapsedMs` is monotonic and the remaining-time derivation is finite.
- **Diagnostics override**: a per-run `MemorySink` at `"trace"` captures events while the
  registry default stays silent; output byte-equals the silent control (the facade-level purity
  complement of M08's test).
- **Horizon**: `ahead: 2` — noteAt(0) realizes 1 and 2 via the policy's requests; the log shows
  them; eviction drops cache but a re-request reproduces byte-identically; a policy whose
  `requestFor` throws (choice-required game) surfaces the error, no partial state.
- **Error taxonomy**: each subclass thrown from its natural site carries structured `details`
  (spot-check three).

## Definition of Done

- [ ] Parity and cancellation-atomicity tests green.
- [ ] No orchestration code imported by any `core` pipeline module (one-way dependency: the
      facade wraps the cores, never the reverse).
- [ ] Full root suite + typecheck green. STOP — hand off. Do not commit.

## Pitfalls

- Async slicing must never reorder RNG consumption — each slice owns its fork; assert by the
  parity test, and never share an `Rng` instance across slice boundaries.
- `shouldYield` may be sync or async — always `await` its result.
