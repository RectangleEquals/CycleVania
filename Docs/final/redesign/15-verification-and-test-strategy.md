# 15 · Verification & test strategy

> The design is only "proven" when every guarantee it makes is a test that runs. This doc is the
> complete invariant inventory — the implementation plan turns each row into a suite — plus the
> fixture datasets, the performance discipline (including a lesson learned the hard way), and the
> human acceptance workflow.

## Invariant inventory

### Determinism ([02](./02-determinism.md))

| Invariant | Test shape |
|---|---|
| Golden-vector parity | RNG/trig/noise outputs byte-equal to pinned `golden/*.json`; regenerated only by an explicit script |
| Full-output determinism | same `(WorldSeed, ReachRequestLog)` → two independent runs → byte-identical serialized descriptors — **including geometry buffers**, and including at least one run with a non-empty modifier history and overrides |
| Fork independence | adding a draw in one subsystem never changes another's output (fixture: generate, then generate again with an extra dummy fork consumed) |
| Preview purity | `previewReachEnvelope(i)` twice in a row → identical; calling it never changes a subsequent `requestReach(i)`'s output |
| Virtual-schedule purity | `computeVirtualSchedule` repeated → identical; consulting it never perturbs real generation |
| Callback purity (double-run) | full generation twice in-process with the same registry object → identical (catches impure host callbacks in shipped presets) |
| CI grep | `Math\.random` / `Math\.(sin|cos|tan|atan)` absent from `core/src` |
| Diagnostics purity | generation under a trace-collecting sink vs. the silent sink → byte-identical output; a throwing sink never corrupts a run |
| Progress monotonicity | `GenProgress.fraction` non-decreasing, ends at 1; async sliced run byte-equals inline (incl. intra-finish slab slicing) |
| Bundle round-trip | export → import → regenerate byte-identical (Inspector and CLI paths both) |

### Solvability & logic ([03](./03-mission-graph.md), [04](./04-worlds-reaches-and-pacing.md))

| Invariant | Test shape |
|---|---|
| Solvability soak | ≥ 1000 seeds × every preset: `isSolvable` true for every generated Reach |
| Solvability locality | Reach *i*'s `validateGraph`/`assumedFill` reads nothing from Reach *i+1*+ (instrumented access assertion) — the concrete proof lazy + provable don't conflict |
| Bonus purity | no progression item in a `bonus`-gated Location; `bonus` Locations excluded from the reachability clause |
| Counted keys | `count(cap, n)` gates: the last copy never behind itself; cross-Reach counts honored via `startHeld` |
| Flag provenance | every rule-referenced flag has exactly one reachable setter; volatile flags never on required conditions (registry rejection test) |
| Bootstrap invariant | hubs offer ≥ items+1 always-reachable Locations for every template in every pool |
| One-way stranding | fuzzed templates with one-way edges: `validateGraph` catches every stranded construction |
| Placement weights | distributional: progression items never in entry Regions, ≤ 1/Region (unless recorded relaxation), measurably vault/depth-biased, sphere-spread |

### Pacing & scheduling ([04](./04-worlds-reaches-and-pacing.md), [05](./05-capabilities-and-facets.md), [06](./06-puzzles-locks-and-recipes.md))

| Invariant | Test shape |
|---|---|
| Budget clamps | `FinalCeiling ≤ ABSOLUTE_HARD_MAX` under any stacked modifiers; rolling-average monotonicity across tiers (never adjacent-pair) |
| Scheduler distribution | high-`powerWeight` entries' mean placement ReachLevel measurably later than low, never impossible early (many-seed histogram) |
| Pity fires | every `guarantee.withinReachLevels` honored in every soak seed; a pity-forced placement never fails `validateGraph` |
| Final sweep completeness | bounded Worlds: every progression capability and required puzzle placed by Reach `L−1`, regardless of drift; the `{1,1}` degenerate case places everything in Reach 0 |
| Pool independence | editing the Puzzle catalog/economy never changes gadget scheduling for the same identity, and vice versa |
| Modifier legality | out-of-depth / excluded-tag / below-`requiredRange` requests rejected as `GenError` |

### Spatial & geometry ([07](./07-spatial-skeleton.md)–[09](./09-naturalization-and-kit.md))

| Invariant | Test shape |
|---|---|
| Envelope overlap | no two Space envelopes overlap after layout (post-resolution assertion, all seeds) |
| Capacity | every L1 edge realized by exactly one connector/socket pair; junction insertions recorded; no edge dropped |
| Socket resolution | every resolved socket lies on the surface (`|field| < ε`), basis orthonormal and fidelity-snapped; every aperture passable (a sphere of the socket's radius fits through — occupancy walk test) |
| Gate fidelity | the rule on every socket/connector descriptor is object-identical to its L1 edge's rule |
| Fidelity | with `angleStepDeg` set: every output normal within ε of the angular grid; with 90°: all normals axis-aligned |
| Kit dedup | `pieces.length ≪ instances.length` (ratio asserted per preset); canonical-yaw dedup verified (rotated identical walls share a piece) |
| Mesh sanity | no NaNs, no unbounded output, index validity, watertight-per-Area within open boundaries (sky-open exempt) |
| Collision | an open cell adjacent to solid stays open under `collideSphere`; a sphere cannot cross a solid wall in a swept walk; OOB is solid |
| Anchors | no two accepted anchors closer than the stricter `minSeparation`; none within `clearanceFromStructural` of a socket; none poking through the hull; every manifest-required anchor placed or `GenError` |
| Outdoor | outdoor Spaces decided at L2 (assertable **without** running geometry); sky-open columns present in occupancy |
| Budgets | poly/unique-piece budgets enforced as `GenError` with offenders named |

### Simulation ([11](./11-simulation-and-autosolve.md))

| Invariant | Test shape |
|---|---|
| Autosolve soak | `autosolve` reaches the terminal for every soak seed × preset — and any divergence from `isSolvable` is treated as the highest-priority bug class |
| Reducer purity | `step` twice from a cloned state → identical results |
| `why` completeness | for every gated link, `why` reports the exact full unmet set (property-tested against rule construction) |

## Fixture datasets

- **The three presets** ([14](./14-dial-reference-and-presets.md)) are the primary soak matrices —
  every suite above runs against all three, which is what proves the fidelity spectrum end to end.
- **The Metroid Prime reconstruction set** — a real shipped game's structure encoded as test
  fixtures (schema-expressiveness stress, not a playable dataset): `mp.gadgets.json` (~22 major
  capabilities + progressive tanks/expansions + a 12-copy facet-less collectathon key —
  exercising combos/`derivedFrom`, resource pools, progressive levels with real shipped
  mechanics), `mp.puzzles.json` (multi-condition boss puzzles, capability-gated bosses, the
  world-scope shrine → boss chain — every taxonomy row instanced), `mp.reaches.json` (per-Reach
  `AreaCountConfig` overrides flexing to a real 64-room / 3-Area region — the range stress test),
  `mp.portals.json` (bidirectional hubs + a one-way prologue — both `ReachPortal` shapes),
  `mp.biomes.json` (five aesthetically distinct packs). The proof: the standard validation
  suite passing against this fixed dataset — solvability on a real game's structure, the
  collectathon guarantee resolving by the declared end, anchor scatter on its densest region.

## Performance & test-cost discipline

Learned directly from a runaway test in the prior implementation (a "do large rooms appear?"
assertion that composed 24 deep Reaches **with geometry on**, running for minutes):

1. **Abstract facts get abstract tests.** Anything decided at L1/L2 (outdoor flags, sizes,
   counts, placement) is asserted with `geometry: false` — this is *why* the layer contract puts
   those decisions before L3 ([01](./01-architecture.md)).
2. **Geometry tests run small.** Fidelity/dedup/collision suites use low-depth Reaches, small
   dims, few seeds; exactly one benchmark test (the `prime` preset, single seed) is allowed to be
   big, marked slow, with an explicit generous timeout.
3. **Distribution tests batch cheaply.** Many-seed statistical assertions always run the cheapest
   configuration that exercises the property.
4. **Perf budgets are tests too**: per-Area finish-pass time and descriptor size ceilings on the
   benchmark, asserted with slack, so regressions surface as failures rather than vibes.

## Acceptance workflow (per milestone)

1. `pnpm -r typecheck` strict + `pnpm test` green (run from the repo root — the workspace requires
   it), including all soaks.
2. `cyclevania soak` + `cyclevania validate` on all presets (the CI gate).
3. Inspector: one-shot build + headless screenshot per changed view, actually viewed; preview
   server and browser killed immediately after.
4. Stop and hand off — the maintainer reviews and commits.
