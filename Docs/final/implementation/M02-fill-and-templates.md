# M02 · Assumed fill & Reach templates

**Goal**: solvability **constructed** — `assumedFill` with the exploration-reward placement
weights, plus the `ReachTemplate` DSL, its interpreter (with gate *binding slots* for content
supplied by the caller), and `ReachTemplatePool`. After this milestone a complete, provably
solvable Reach graph can be generated from data.

**Required reading**: [redesign 03](../redesign/03-mission-graph.md) — “Assumed fill”,
“Placement weights”, “Bootstrap invariant”, “ReachTemplate”, “Edge cases”; the pipeline order in
[redesign 04 §The Reach-generation pipeline](../redesign/04-worlds-reaches-and-pacing.md)
(selection precedes interpretation — interpretation receives already-selected content).

**Prerequisites**: M01 green.

**Porting references**: legacy `core/src/fill/{assumed-fill,fill-policy}.ts`,
`core/src/template/{template,grammar,role}.ts` (the grammar is the closest proven analogue of the
interpreter — its bootstrap/gating/loop logic is worth mining; its shape differs, spec wins).

## Phase 2.1 — `core/src/fill/`

`placement-weights.ts` — `PlacementWeightConfig` exactly as in redesign 03 (fields + defaults:
`entrySpaceWeight 0`, `depthExponent 1.5`, `vaultBonus 2.0`, `behindGateBonus 1.5`,
`perRegionCap 1`, `sphereSpreadBonus 1.5`) and:

```ts
export function locationWeight(loc: CandidateLocation, ctx: WeightContext, cfg: PlacementWeightConfig): number;
```

where `CandidateLocation` carries `{ locationId, regionId, regionRole, depthRank, behindGateCount }`
and `WeightContext` carries `{ progressionPlacedInRegion: Map<RegionId, number>, lastPlacementSphereHint }`.
Weight = product of applicable factors; 0 removes the candidate. **Relaxation order** when the
candidate set is empty: drop `perRegionCap` first, then `entrySpaceWeight`; each relaxation is
appended to a `relaxations: string[]` out-parameter (never silent).

`assumed-fill.ts`:

```ts
export interface FillResult { placement: Map<LocationId, string /*itemId*/>; relaxations: string[]; }
export function assumedFill(graph: MissionGraph, startHeld: Held, items: Item[],
                            rng: Rng, weights: PlacementWeightConfig): FillResult;
```

Algorithm exactly per redesign 03: progression items first (descending `powerWeight` if provided,
else input order); for each, `held = startHeld + every other unplaced item's grants`; candidates =
reachable, empty, non-`bonus`-gated Locations; weighted seeded pick via `rng.weighted`; then
`fillRemaining` drops `useful`/`filler`/`bonus` items into leftover Locations (bonus-gated
Locations allowed for non-progression only). Finish by asserting `isSolvable(...)` — if false,
**throw a plain Error** (internal bug — the construction guarantees it true).

## Phase 2.2 — `core/src/template/`

`reach-template.ts` — `ReachTemplate`, `BranchSpec`, `gating`, `loops` exactly per redesign 03,
plus `ReachTemplatePool` (`poolAt(depth)`).

`interpret.ts`:

```ts
export interface SelectedContent {
  gateRules: Rule[];          // rules to bind onto gate slots, in priority order (M04 builds these
                              // from scheduled capabilities/puzzles + teach→test→combine pacing;
                              // until M04, tests pass hand-built rule lists)
  progressionCount: number;   // for the bootstrap invariant (≥ progressionCount + 1 hub slots)
}
export interface StructureNudges { extraBranchChance?: number; extraLoopChance?: number; }
export function interpretTemplate(template: ReachTemplate, content: SelectedContent,
                                  nudges: StructureNudges, rng: Rng): MissionGraph;
```

Steps, in order (each seeded from `rng` in this fixed sequence — order is contractual):
instantiate nodes → wire the spine → hang branches (base chance + `extraBranchChance`) → gate
`lockFraction` of spine edges, consuming `content.gateRules` in order (a `gate`-roled node always
locks its entrance first; respect `keepEntryOpen`/`keepExitOpen`; if rules run out, remaining
gate slots stay open — never invent a rule) → close loops (`guaranteeAtLeastOne`, then
`density` + `extraLoopChance` extra attempts) → allocate Location slots per node within each
node's `slots {min,max}` → **validate the bootstrap invariant** (hub Regions offer ≥
`progressionCount + 1` always-reachable Locations; violation ⇒ `GenError` naming the template) →
register `FlagDef`s for every flag any bound rule references, `setBy` pointing at a Location in
the rule's own gate-adjacent Region (the puzzle layer refines provenance in M04).

`template-pool.ts` — seeded weighted draw from `poolAt(depth)`.

## Phase 2.3 — Tests

`fill/fill.test.ts`:
- **Solvability soak**: a synthetic template (hub + 4 segments + gate + capstone + terminal + 2
  vaults + a loop) × 1000 seeds × 4 hand-built items: `interpretTemplate` → `assumedFill` →
  `isSolvable` true for every seed; zero throws.
- **Placement distribution** (same soak, aggregate): progression items never in the hub's entry
  Region (unless a recorded relaxation fired — assert relaxations empty in this fixture); ≤1
  progression item per Region; vault Regions receive measurably more than uniform share.
- Counted keys: 3 copies of one id against a `count(k,3)` gate — solvable every seed.
- Bonus purity: a progression item is never placed in a `bonus`-gated Location (fixture with
  tempting bonus slots).
- Self-gating canary: assert no item's Location is gated on its own grant (walk placements).

`template/template.test.ts`:
- Gate binding: supplied `gateRules` appear on edges verbatim (object equality), in priority
  order, `gate` node entrance first; surplus slots stay `ALWAYS`.
- Loops: `guaranteeAtLeastOne` yields ≥1 back-edge every seed; `density: 1` yields strictly more
  loops on average than `density: 0` (two-sample comparison over 200 seeds).
- Bootstrap violation: a template with 1 hub slot + `progressionCount: 3` throws `GenError`
  mentioning the template id.
- One-way stranding: a hand-made template whose branch is entered by a one-way and has no exit ⇒
  `validateGraph` throws (wire this fixture through interpret → validate).
- Pool: `poolAt` draw is deterministic per seed and respects weights distributionally.

## Definition of Done

- [ ] 1000-seed soak green and fast (< ~10 s — this fixture has no geometry, no heavy math).
- [ ] All relaxations surfaced via `FillResult.relaxations`; none silent.
- [ ] `interpretTemplate` never invents a rule and never consumes RNG outside the fixed step
      sequence.
- [ ] Full root suite + typecheck green. STOP — hand off. Do not commit.

## Pitfalls

- The fill loop's “held = everything else unplaced” must rebuild per placement — caching a
  `Held` across placements silently breaks the induction.
- Draw order inside interpret is part of determinism: adding a new seeded step later means
  bumping `generationVersion` (M08) — structure the function so steps are clearly delimited.
