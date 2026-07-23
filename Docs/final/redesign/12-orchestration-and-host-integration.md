# 12 · Orchestration & host integration

> The composers are pure synchronous cores. Orchestration is a thin facade that schedules those
> same cores — cancellation, progress, prefetch, worker offload — without ever changing their
> output: an orchestrated generation is byte-identical to an inline one. This doc also carries the
> complete host-integration surface and the error taxonomy.

## Sync cores, async facade — and async is the host default

```ts
// the pure cores (everything in docs 03–09):
createWorld(registry: Registry, seed: string, config: WorldConfig): WorldComposer   // cheap; generates nothing
worldComposer.requestReach(request): ReachComposer                                   // one full Reach, synchronously

// the facade:
requestReachAsync(world, request, hooks?): Promise<ReachDescriptor>
interface OrchestrationHooks {
  onProgress?(p: GenProgress): void;
  shouldYield?(): Promise<void> | void;    // host-pure cooperative yielding (frame budget)
  token?: CancellationToken;               // cancel → rejects with GenCancelled; no partial output escapes
  diagnostics?: DiagnosticsConfig;         // per-run override of the registry-level config (below)
}
```

**Hosts should treat the async facade as the default entry point.** The synchronous cores exist
as the determinism/testing substrate; anything player-facing runs through the facade so frames
stay responsive during long generations. The facade slices work at natural boundaries — per
phase, per Area (Areas are independent forks by construction, [01](./01-architecture.md)) —
**and inside the long phases**: the finish pass (the dominant cost) is structured as a resumable
slab iterator, so meshing a large Area yields every z-slab rather than blocking for the whole
Area. Determinism holds because slicing never reorders RNG consumption — each slice owns its own
fork, and a sliced run is byte-identical to an inline one.

### Progress reports — structured enough for a real loading screen

```ts
type GenPhase = "template" | "selection" | "graph" | "fill" | "skeleton"
              | "volume" | "anchors" | "finish" | "assemble";

interface GenProgress {
  phase: GenPhase;
  label: string;                 // human-readable: "Meshing Area 3/5"
  areaIndex?: number; areasTotal?: number;
  fraction: number;              // 0..1 OVERALL, monotonic non-decreasing across the whole run
  phaseFraction: number;         // 0..1 within the current phase
  elapsedMs: number;             // remaining ≈ elapsedMs * (1 - fraction) / fraction
}
```

Overall `fraction` is computed against a static per-phase **weight table** (finish dominates;
weights are part of the dial surface) so the bar moves honestly rather than sprinting through
cheap phases and stalling at 90%. The host decides presentation entirely — a custom loading
screen, a diegetic animation, a terminal line; the Inspector ships a plain default overlay
rendering exactly this stream ([13](./13-inspector-and-tooling.md)), so "what the Inspector
shows" and "what a host can show" are the same data.

## Diagnostics & logging — one channel, every package

Errors alone aren't enough for either audience — a host integrating CycleVania needs actionable
warnings; a CycleVania developer needs to watch the generator think. One structured,
**write-only** channel serves both:

```ts
type DiagLevel = "error" | "warn" | "info" | "debug" | "trace";

interface DiagEvent {
  level: DiagLevel;
  code: string;                  // stable, grep-able: "fill.relaxed-per-region-cap",
                                 // "skeleton.junction-inserted", "finish.res-clamped", …
  message: string;               // human-readable
  path?: string;                 // where: "reach3/area:r2/space:s4"
  details?: Record<string, unknown>;
}
interface DiagnosticsSink { emit(e: DiagEvent): void; }
interface DiagnosticsConfig { level: DiagLevel; sink: DiagnosticsSink; }   // default: level "warn", silent sink
```

Rules, all load-bearing:

1. **Diagnostics never affect generation.** Output is byte-identical under any sink and any
   level — enforced by test (generate under a trace-collecting sink vs. the silent sink;
   compare). Sinks must not throw into the pipeline (the emitter guards).
2. **Codes are stable API.** Every recorded relaxation ([03](./03-mission-graph.md),
   [07](./07-spatial-skeleton.md), [09](./09-naturalization-and-kit.md)) emits a `warn` with the
   same code string that lands in `ReachMeta.relaxations`; every `GenError` emits an `error`
   event (same code + details) before throwing; registry validation emits `warn` for soft issues
   (unused vocabulary tags, never-referenced catalog entries) alongside its hard `GenError`s.
   `info` marks phase transitions; `debug`/`trace` narrate choices (scheduler rolls, layout
   iterations) for CycleVania's own developers.
3. **Configurable everywhere.** The registry carries a default `DiagnosticsConfig`; every
   generation call accepts a per-run override; the CLI exposes `--log-level`; the Inspector
   renders the channel visually in its diagnostics panel and exposes the level as a dial
   ([13](./13-inspector-and-tooling.md)). Core itself never writes to a console — sinks are
   always supplied by the tool or host.

## Horizon prefetch — eager *requests*, still lazy *generation*

Reaches are requested, never scheduled ([04](./04-worlds-reaches-and-pacing.md)) — but a host
streaming a live game wants the next Reach ready before the player commits. The
`GenerationHorizon` helper reconciles the two by being explicitly **the host's agent issuing real
requests early**:

```ts
interface HorizonPolicy {
  ahead: number;                            // keep N Reaches realized past the player's position
  requestFor(nextIndex: number): ReachRequest;   // HOST-authored: the horizon can't invent modifier
                                            // choices, so the host must say what a prefetched
                                            // request contains (e.g. "no modifiers until chosen")
  evictBehind?: number;                     // drop realized descriptors (they're regenerable) beyond M behind
}
```

The horizon calls `requestReachAsync` with host-authored requests, caches descriptors, and evicts
by policy. Because prefetched requests enter the `ReachRequestLog` like any other, determinism is
untouched. **The caveat is real and by design**: a game whose fiction requires the *player* to
choose modifiers before a Reach exists cannot prefetch that Reach — the horizon is for hosts whose
requests are choice-free (or whose choices are known early), and `previewReachEnvelope` (pure,
free) is the tool for everything before commitment.

## Worker offload

`WorkerLike` adapter: a Reach (or single Area) generation job is `(registrySnapshot | fingerprint,
seed, request) → descriptor` — pure data in, pure data out, so it runs on a Web Worker or a server
thread unchanged, and the byte-identical guarantee is testable by running the same job inline.

## Error taxonomy

| Class | Meaning | Examples | Contract |
|---|---|---|---|
| `GenError` (typed, subclassed) | **Host data is wrong** — actionable by the host | stranded Region (with the stranded set + frontier rules); unsettable required flag; required anchor unplaceable (Space + kind named); volatile flag on a required condition; orphan vocabulary tag; poly/piece budget exceeded (worst offenders listed); modifier illegal at depth; illegal `ReachRequest` slot | thrown from `defineRegistry`/`requestReach`; message names the exact registry entry/dial to fix; never partially applied |
| `GenCancelled` | cooperative cancellation | token fired mid-generation | facade-only; cores never see it |
| plain `Error` (throw) | **CycleVania bug** — internal contradiction | `isSolvable` false after `assumedFill`; autosolve failing where `isSolvable` passed; QEF NaN | not catchable-and-continue by design; every one is a fixable defect |

Nothing is silently dropped anywhere in the pipeline. Where the design allows a documented
relaxation (placement-cap relaxation [03](./03-mission-graph.md); junction insertion, connector
waypoints [07](./07-spatial-skeleton.md); `maxDim` coarsening [09](./09-naturalization-and-kit.md)),
it is *recorded in meta*, so tooling can display every deviation from the ideal.

## The host-integration surface (complete)

The registries — deliberately small; if a host needs to reach past these, that's a sign a new
named contract belongs here, not a one-off hook:

| Registry / config | Doc | One-line purpose |
|---|---|---|
| `GadgetCatalog` (`CapabilityDef[]` + `GadgetDef[]`) | [05](./05-capabilities-and-facets.md) | capabilities, Facets, powerWeight, pity |
| `GadgetEconomyConfig` | [05](./05-capabilities-and-facets.md) | progression items per Reach `{min,max}` |
| `PuzzleCatalog` (`PuzzleDef[]`) | [06](./06-puzzles-locks-and-recipes.md) | puzzles/locks: scope, class, condition, outcome |
| `PuzzleEconomyConfig` | [06](./06-puzzles-locks-and-recipes.md) | puzzles per Reach `{min,max}` |
| `SpatialRecipeDef` set + `LockVocabulary` | [06](./06-puzzles-locks-and-recipes.md) | how locks become space; named lock patterns |
| `LockPacingConfig` | [06](./06-puzzles-locks-and-recipes.md) | teach → test → combine |
| `ReachTemplatePool` | [03](./03-mission-graph.md) | depth-scoped weighted macro-shapes |
| `WorldLengthPolicy` · `AreaCountConfig` | [04](./04-worlds-reaches-and-pacing.md) | ranged counts, drawn once |
| `ReachModifierCatalog` + `ReachModifierPolicy` | [04](./04-worlds-reaches-and-pacing.md) | risk/reward dial patches + the mandatory ramp |
| `ComplexityConfig` + hazard/reward baselines | [04](./04-worlds-reaches-and-pacing.md) | the curve constants |
| `PlacementWeightConfig` | [03](./03-mission-graph.md) | exploration-reward placement shape |
| `BiomePack` set + `BiomePlanConfig` | [08](./08-volumetric-composition.md) | palettes, materials, hazards, dressing, noise, transitions |
| `HullArchetypeDef` set | [08](./08-volumetric-composition.md) | procedural + authored-landmark hull recipes |
| `ContentAnchorKind` set | [08](./08-volumetric-composition.md) | per-kind scatter density/separation/clearance |
| `SignatureConfig` · connector length bounds · degree table | [07](./07-spatial-skeleton.md) | socket compatibility + wiring dials |
| `FidelityProfile` | [09](./09-naturalization-and-kit.md) | the spectrum knob |
| geometry budgets (`polyBudgetPerArea`, `maxUniquePieces`) | [09](./09-naturalization-and-kit.md) | memory ceilings |

`defineRegistry(input)` validates all of it in one pass — cross-referencing the shared tag
vocabulary (orphans are errors), checking rule/flag provenance, volatile-flag legality, template
bootstrap invariants, and fingerprinting the result ([02](./02-determinism.md)).

### Minimal host example

```ts
import { defineRegistry, createWorld } from "@cyclevania/core";
import { classicPreset } from "@cyclevania/examples";       // or your own data (14)

const registry = defineRegistry({ ...classicPreset, gadgets: myGadgetCatalog });
const world = createWorld(registry, "expedition-7", { geometry: true });

const reach0 = world.requestReach({ reachIndex: 0, chosenModifiers: [] });
// realize: reach0.descriptor → your realizer (10). Later, from in-game triggers:
const preview = world.previewReachEnvelope(1);              // show the player risk/reward
const reach1 = world.requestReach({ reachIndex: 1, fromReachIndex: 0, chosenModifiers: picked });
```

### The "never assumes gameplay" checklist

Before adding anything to CycleVania itself: does it require knowing what a capability *does*,
what a puzzle *is*, what a biome *looks like*, what a modifier is called in-fiction, or what
triggers a request? If yes, it belongs in a registry entry or the host's trigger code — not in
CycleVania's code, docs, or type names. Every doc in this suite stays readable, and every
algorithm testable, without one real gadget/puzzle/biome name appearing.

### Host obligations (the short contract)

1. Convert your units inside Facet callbacks — CycleVania's units only ever cross the boundary
   ([05](./05-capabilities-and-facets.md)).
2. Keep callbacks pure; bump `revision` when their behavior changes ([02](./02-determinism.md)).
3. Treat descriptors as read-only; realize, don't mutate.
4. Ship the same registry (fingerprint-checked) to every peer that regenerates from identity
   ([10](./10-output-contract.md)).
5. Route every "make the world different" wish through a dial or registry entry — and if none
   fits, request a new named contract rather than patching output.
