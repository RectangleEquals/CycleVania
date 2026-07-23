# M04 · Capabilities, Puzzles & the registry surface

**Goal**: the two first-class content pools — Facet-based capabilities and the Puzzle pool —
their shared scheduler (eligibility + pity + virtual-schedule bias + economies + final sweep),
spatial recipes, the full `defineRegistry` validation surface with vocabulary cross-referencing
and fingerprinting, and the real `ContentSelector` wired into `WorldComposer` (replacing M03's
verbatim strategy).

**Required reading**: [redesign 05 (all)](../redesign/05-capabilities-and-facets.md),
[redesign 06 (all)](../redesign/06-puzzles-locks-and-recipes.md),
[redesign 12 §The host-integration surface](../redesign/12-orchestration-and-host-integration.md),
[redesign 02 §Registry fingerprint](../redesign/02-determinism.md).

**Prerequisites**: M03 green.

## Phase 4.1 — `core/src/capability/`

`capability-def.ts` — `CapabilityId`, `CapabilityDef` (with `held`, `facets`, `powerWeight`,
`guarantee?`, `category?`, `revision?`), `GadgetDef`, `FacetContext`, and the three Facet shapes —
all **exactly** as typed in redesign 05.

`facets.ts`:

```ts
export const BUILTIN_BUCKETS: readonly string[];   // the 9 from redesign 05's table
export function buildHeld(defs: Map<CapabilityId, CapabilityDef>, grants: Map<CapabilityId, number>,
                          flags: Set<string>): Held;      // resolves derivedFrom membership (+ minLevels)
export function aggregateBuckets(defs, held): Record<string, number>;
// sums MagnitudeFacet.evaluate({level, resource: {charge: capacity, capacity}, held}) per bucket,
// fixed iteration order (catalog order), including derived capabilities that are held
export function activeTags(defs, held): Set<string>;      // TagFacet.tag where evaluate() !== false
```

`economy.ts` — `GadgetEconomyConfig` (default `{min: 1, max: 3}`); `resolveEconomy(base,
modifierPatches, requestOverride)` (stacking order per redesign 05: modifiers first, then the
request override).

## Phase 4.2 — the shared scheduler: `world/scheduling.ts`

One generic engine, two thin consumers (this satisfies redesign 01's layout — the pool-specific
files below wrap it):

```ts
export interface SchedulableEntry { id: string; powerWeight: (level: number) => number;
                                    guarantee?: { withinReachLevels: number }; }
export interface ScheduleContext { reachLevel: number; virtualPlan?: Map<string, number>;
                                   reachIndex: number; isFinalReach: boolean;
                                   placedLevels: Map<string, number>;     // id → grants so far
                                   firstEligibleLevel: Map<string, number>; }
export function eligibility(reachLevel: number, powerWeight: number,
                            levelsSinceEligible: number, guarantee?: {withinReachLevels: number}): number;
// the logistic from redesign 05 — constants MAX_LEVEL_SHIFT = 6, SOFTNESS = 1.25
export function scheduleDraw(pool: SchedulableEntry[], count: {min: number; max: number},
                             ctx: ScheduleContext, rng: Rng):
  { chosen: string[]; pityForced: string[]; sweepForced: string[] };
```

`scheduleDraw`: draw the count in `[min, max]` (seeded, biased upward with `reachLevel`);
candidate weight = `eligibility(...)` × (virtual-plan bonus ×8 when `virtualPlan.get(id) ===
reachIndex`); weighted draws without replacement; then append pity-forced entries (eligibility
returned 1 via guarantee) exempt from `max`; then, if `isFinalReach`, sweep-force every remaining
progression/required entry (also exempt). Next-level grants of already-held entries participate
with `powerWeight(nextLevel)`.

`capability/gadget-scheduler.ts` and `puzzle/puzzle-scheduler.ts` wrap `scheduleDraw` with their
own fork (`${reachRoot}:gadget-schedule` / `${reachRoot}:puzzle-schedule`), their own catalog
filter (progression capabilities / all puzzles, `required` swept), and their own economy.

## Phase 4.3 — `core/src/puzzle/`

`puzzle-def.ts` — `PuzzleDef`, `PuzzleScope`, `PuzzleClass`, `PuzzleOutcome`, `EdgeSpec`,
`ItemSpec` exactly per redesign 06. `PuzzleInstance` = `{ instanceId, defId, boundRegion?,
boundEdge?, condition (the def's Rule object, shared by reference) }`.

`recipes.ts` — `SpatialRecipeDef` exactly per redesign 06, plus `SHIPPED_RECIPES`: the 14 named
archetypes from redesign 06 with sensible parameters (e.g. `arena`: kinds `["room","outdoor"]`,
minExtent `[10,10,4]`, anchors `[{kindId:"interactable", count:{min:0,max:0}}]` + lockdown
handled by condition; `panel-array`: 3–5 `interactable` anchors; `gap-crossing`: carve
`"hazard-trench"`, scaleWith `"depth"`; …). Keep parameters modest — presets refine them.

`lock-vocabulary.ts` — named pattern builders for the 15-row taxonomy
(`capabilityLock(cap)`, `switchLock(flagName)`, `panelArray(flags)`, `arenaLockdown(id)`,
`collectathon(cap, n)`, …) each returning a partial `PuzzleDef` the host completes.

`outcomes.ts` — pure helpers applying an outcome to a graph-under-construction
(`open-edge` adds/unlocks an edge; `spawn-item-here` registers an extra Location;
`grant-capability` marks the instance as a grant source; `set-flag-only` registers the FlagDef
with `setBy: instanceId`). Used by interpretation binding below.

## Phase 4.4 — `core/src/registries/define-registry.ts`

`RegistryInput` (every table row from redesign 12) → validated `Registry`. Validation list
(each failure a `GenError` naming the entry and field):

1. Unique ids per catalog; `GadgetDef.grants` refer to existing capabilities; `derivedFrom`
   refers to existing, acyclic capabilities.
2. **Vocabulary cross-reference**: collect every tag/signature string from TagFacets, recipe
   `sockets[].signature` + `anchors[].tags`, `ContentAnchorKind.id`s, socket-signature configs;
   every *referenced* tag must be *declared* by at least one producer (orphan ⇒ error listing the
   tag and its referencer).
3. `required`-class `PuzzleDef.condition` must not `usesVolatileFlag` (M01 helper).
4. Every `PuzzleDef.spatialRecipe` names an existing recipe; recipe `space.kinds` non-empty.
5. Economies: `1 ≤ min ≤ max`; `WorldLengthPolicy.min ≥ 1 ≤ max`; modifier `minDepth ≥ 0`;
   templates pass a static shape check (criticalPath ids exist in nodes; ≥1 hub; last node
   capstone-then-terminal).
6. Capability referenced by any Lock/recipe/`count` exists in the catalog (registry-fixed rule).
7. Compute `registryFingerprint` = fnv1a over a canonical stable-JSON serialization (sorted
   keys; callbacks contribute their `revision ?? 0` — document this in a comment).
8. **Soft issues emit `warn` diagnostics** (M00's channel), never errors: declared-but-never-
   referenced vocabulary tags, catalog entries no template/recipe/lock can ever place, a
   `guarantee` window larger than the World's max length. Codes: `registry.unused-tag`,
   `registry.unreachable-entry`, `registry.guarantee-exceeds-length`. `defineRegistry` accepts an
   optional `DiagnosticsConfig` and the validated `Registry` carries it as the world default.

Config-shape files (`biome-pack.ts`, `hull-archetypes.ts`, `fidelity.ts`, `anchor-kinds.ts`,
`signature.ts`, …) hold the *types* + shipped defaults; their consumers arrive in M05–M07 — type
them now exactly per redesign 07–09 so the registry is complete once.

## Phase 4.5 — The real `ContentSelector` + gate binding

`world/content-selector.ts` implements M03's `ContentSelector` interface:

1. Run both schedulers (economies resolved per Phase 4.1) → chosen capabilities (as `Item`s:
   `{ id: gadgetId, class: "progression", grants }` — plus filler/useful items from the registry's
   loot lists) and chosen `PuzzleInstance`s.
2. Build `gateRules` for interpretation: order per **teach → test → combine**
   (`LockPacingConfig`): first a simple `have(newCap)` for the Reach's newly-introduced
   capability, then puzzle-instance conditions by ascending recipe difficulty, `combineChance`
   composing late rules with an earlier-sphere `have` via `and`. **In-scope check**: every
   `required` rule's caps ⊆ `startHeld ∪ selected` — violation demotes the instance to
   `optional-*` (recorded in relaxations) per redesign 06's coupling invariant.
3. Apply puzzle outcomes to the graph via `outcomes.ts` during interpretation binding (extend
   `interpretTemplate`'s `SelectedContent` to carry instances; refine `FlagDef.setBy` to the
   instance id).

Replace the default selector in `createWorld` with this one (the verbatim selector remains
exported for tests).

## Phase 4.6 — Tests

- `capability/facets.test.ts` — derived held (combo held only when both prerequisites at
  minLevels); bucket aggregation (progressive level: `evaluate: ctx => ctx.level * 2` sums
  correctly at level 3); `activeTags` honors `evaluate?`; resource capacity feeds context.
- `world/scheduling.test.ts` — distribution: over 400 seeds, mean placement ReachLevel of a
  0.9-power entry strictly greater than a 0.2-power entry, and the 0.9 entry appears at level 0
  in at least one seed; pity: `withinReachLevels: 2` always placed by then, exempt from `max`;
  sweep: bounded world places everything by `L−1`; `{min:1,max:1}` world places everything in
  Reach 0.
- `puzzle/puzzle.test.ts` — every taxonomy builder produces a valid def; volatile-on-required
  rejected by `defineRegistry`; world-scope collectathon: N facet-less copies + `count` shrine
  across a 4-Reach world → solvable, shrine satisfiable by the end (integration mini-soak).
- `registries/registry.test.ts` — each validation rule above has a failing fixture asserting the
  `GenError` message names the entry; fingerprint changes when any entry or `revision` changes
  and is stable otherwise.
- `world/selector.test.ts` — teach→test→combine ordering of bound gates; in-scope demotion
  recorded; pool independence (edit puzzle catalog ⇒ gadget outcomes byte-identical, and vice
  versa — the redesign 15 invariant).

## Definition of Done

- [ ] `createWorld(...).requestReach(...)` now runs with **zero caller-supplied content** — the
      registry + schedulers drive everything; M02/M03 suites still green unmodified.
- [ ] Pool independence and preview purity still green (they guard this milestone's wiring).
- [ ] Every `GenError` fixture asserts on message content, not just throw.
- [ ] Full root suite + typecheck green. STOP — hand off. Do not commit.

## Pitfalls

- The virtual-plan bonus multiplies eligibility — it must never resurrect a zero (an ineligible
  guarantee-less entry stays ineligible).
- `scheduleDraw` draws **without replacement** — removing a chosen candidate must not reindex the
  RNG consumption of the remaining draws in a way that depends on array identity (draw by
  cumulative weight over the live candidate list).
- Share `condition` Rule objects by reference between `PuzzleInstance` and the bound edge — the
  redesign's "one object, one source of truth" coupling invariant; M05's gate-fidelity test
  checks object identity (`===`).
