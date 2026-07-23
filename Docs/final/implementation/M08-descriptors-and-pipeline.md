# M08 · Descriptors & the wired pipeline

**Goal**: the `descriptors/` module — every output shape from the contract, canonical
serialization, meta/provenance — and the completed master sequence: `requestReach` now returns a
full `ReachDescriptor` (skeleton + volume + anchors + optional geometry), and a `WorldDescriptor`
assembles everything. This is the milestone where the whole-output determinism guarantee becomes
a test.

**Required reading**: [redesign 10 (all)](../redesign/10-output-contract.md);
[redesign 02 §Registry fingerprint, §What can legitimately change a world](../redesign/02-determinism.md).

**Prerequisites**: M07 green.

## Phase 8.1 — Shapes: `descriptors/shapes.ts`

Transcribe the descriptor tree from redesign 10 **exactly**: `WorldDescriptor`, `WorldMeta`,
`ReachDescriptor`, `ReachMeta`, `MissionGraphData`, `PlacementData`, `PuzzleInstanceData`,
`AreaDescriptor`, `SpaceDescriptor`, `SocketData`, `ConnectorDescriptor`, `PortalSpec`,
`ContentAnchor` (with the binding union), `OccupancyData`, plus `RuleData` (the Rule union is
already JSON-safe — alias it), `HeldData`, `WorldBoxData`, `Vec3Data`. Optional fields are
**absent, never null** (`exactOptionalPropertyTypes` enforces this — lean on it).

Set `GENERATION_VERSION = "1.0.0"` in `descriptors/version.ts` — bumped by any future
output-altering algorithm change (add a comment citing redesign 02).

## Phase 8.2 — Assembly: `descriptors/assemble.ts`

Pure converters from internal results to descriptor shapes:
`assembleReach(reachResult, meta): ReachDescriptor`, `assembleWorld(worldComposer):
WorldDescriptor`. `ReachMeta` carries everything redesign 10 lists — `requestIdentity`,
`chosenModifiers`, `finalCeiling`, `areaCount`, `buckets`, `spheres`, `relaxations` (the union of
every recorded relaxation from fill/skeleton/finish), `startHeld`. `WorldMeta` carries
`worldSeed`, `generationVersion`, `registryFingerprint`, `lengthPolicy`, `drawnLength?`,
`requestLog`.

## Phase 8.3 — Canonical serialization: `descriptors/serialize.ts`

```ts
export function stableStringify(x: unknown): string;   // schema-order keys, deterministic arrays
export function toTypedKit(kit: GeneratedKit): TypedKit; // Float32Array/Uint32Array views (host helper)
```

`stableStringify` emits object keys in **declaration order of the shapes** (practically: build
descriptor objects with keys in schema order and serialize with plain `JSON.stringify`; add a
dev-mode assertion helper that key order matches the schema list). Round-trip law:
`JSON.parse(stableStringify(d))` deep-equals `d`, and re-stringifying is byte-identical.

## Phase 8.4 — Finish wiring the master sequence

`requestReach` end-state (steps 1–16 of [redesign 01](../redesign/01-architecture.md), selection
before interpretation): request validation → template → ceiling → selection → interpretation →
validate → fill → skeleton (M05) → volume + sockets + anchors (M06) → finish if `geometry`
(M07) → assemble → return `ReachDescriptor` (also retained on the composer). Per-Area stages run
from per-Area forks in Region order — restructure now so each Area's stages 8–15 are one pure
function call (`composeArea(areaInputs): AreaDescriptor`) — M10's worker offload depends on this
boundary.

## Phase 8.5 — Tests (`descriptors/output.test.ts`)

- **Whole-output determinism** (the flagship test): one seed, 2 Reaches, modifiers on Reach 1, an
  economy override, `geometry: true`, small fidelity dims — generate twice in fresh composers →
  `stableStringify(worldA) === stableStringify(worldB)` byte-identical. Repeat with
  `geometry: false`.
- **Additivity**: `geometry: false` then a later `finishArea` upgrade on the same seed produces
  exactly the geometry the `geometry: true` run had.
- **Round-trip**: parse → re-stringify byte-identical; typed-kit conversion preserves values.
- **Meta completeness**: fingerprint changes with a registry edit; `requestIdentity` echoes into
  the right Reach; every recorded relaxation from a junction-forcing fixture appears in
  `ReachMeta.relaxations`.
- **Sparseness**: requesting slots 0 and 2 (branching world fixture) yields exactly two realized
  Reaches, nothing about slot 1 anywhere in the output.
- **Rule identity survives assembly**: the serialized gate on a socket deep-equals the L1 edge's
  serialized rule (serialization can't preserve `===`, so this is the deep-equal complement of
  M05's identity test).
- **Diagnostics purity, full-pipeline** (the promised M00 follow-through): the flagship
  determinism fixture generated once under `SILENT_SINK` at level `"warn"` and once under a
  `MemorySink` at level `"trace"` → byte-identical descriptors; the trace run's events all carry
  populated `path`s; a deliberately-throwing sink neither corrupts nor aborts the run.

## Definition of Done

- [ ] `createWorld → requestReach → assembleWorld` is the complete public happy path, exported
      from `core/src/index.ts` with every public type from redesign 10.
- [ ] The flagship determinism test is green and takes < ~30 s.
- [ ] `composeArea` is a standalone pure function (M10's offload seam).
- [ ] Full root suite + typecheck green. STOP — hand off. Do not commit.

## Pitfalls

- Build every descriptor object literally with keys in schema order — retrofitting key order
  later touches every assembler.
- `Map`/`Set` never appear in descriptor shapes — convert at assembly (plain objects/arrays
  only).
- Do not store live `Rng` or callbacks on any result object the assembler can reach.
