# M11 · Examples & presets

**Goal**: `@cyclevania/examples` — the three shipped presets (`crawler`, `classic`, `prime`) as
complete loadable registries, the example content catalogs, and the Metroid Prime-scale fixture
set. From here on, the soak matrix runs against real datasets, which is what proves the fidelity
spectrum end to end.

**Required reading**: [redesign 14 (all)](../redesign/14-dial-reference-and-presets.md);
[redesign 15 §Fixture datasets](../redesign/15-verification-and-test-strategy.md); the example
catalogs in the prior-art docs (below).

**Prerequisites**: M09 and M10 green.

## Phase 11.1 — Package scaffold

`packages/examples/`: `package.json` (`@cyclevania/examples`, dependency: `@cyclevania/core` via
`workspace:*`, `"main": "./src/index.ts"`), tsconfig, `src/index.ts` barrel. Tests in this
package run under the root vitest config automatically.

## Phase 11.2 — Example content catalogs

- `src/gadget-catalog.ts` — the 24-verb example catalog. Port the names/caps/verbs from the
  prior-art table (`Docs/09-gadget-and-lock-cookbook.md` in the legacy docs, retrievable from the
  working tree — it still exists on disk) and express each as a modern `CapabilityDef` +
  `GadgetDef`: Facets per verb (e.g. `leap` → Magnitude `traversal.zUp`; `reveal` → Tag
  `revealable`; `small-form` → Tag `crawl-aperture`; `rappel` → Magnitude `traversal.zDown`;
  `ward` → Tag per hazard; boss verbs → `challenge.offense` + boss tags), `powerWeight` per the
  legacy `power` values (grapple 0.4, leap 0.7, phase 0.75, recall 0.72, invert 0.85, magnet
  0.35, reveal 0.68, small-form 0.3, translate 0.2 — assign sensible values for the rest),
  `guarantee` on 3–4 flagship entries.
- `src/puzzle-catalog.ts` — at least one `PuzzleDef` per taxonomy row (15+), built via the
  `lock-vocabulary` builders, plus an Area-scope fragment collectathon and a world-scope
  collectathon chain (shrine + boss, per redesign 06).
- `src/biomes.ts` — 3 example `BiomePack`s (enclosed-stone, flooded-cavern, open-highland) with
  palettes, materials per surface, hazard sets, dressing sets, noise, `outdoorAffinity`.
- `src/hulls.ts` — example hull archetypes beyond core's shipped set + **one authored landmark**
  (an SDF composition distinctive in silhouette — e.g. dome + spire + terraces).
- `src/templates.ts` — 3 templates (linear-ish, hub-and-spoke, loop-heavy) + a
  `ReachTemplatePool` mixing them by depth.
- `src/modifiers.ts` — an 8-entry modifier catalog with a `requiredRange` ramp (optional → 1
  mandatory at depth 8+).

## Phase 11.3 — The three presets: `src/presets/{crawler,classic,prime}.ts`

Each exports a complete `RegistryInput` per redesign 14's specification — dial values verbatim
from that doc (crawler: 90°/box-only/zero-noise/no-outdoor/no-landmarks/lateral;
classic: 5°/defaults/1 landmark; prime: flexing AreaCounts/2 landmarks + vistas/outdoor bowls/
sub-biome gradients/deep z/world collectathon/mandatory-modifier ramp). All three
`defineRegistry` clean. `classic` is the default export the inspector will open with.

## Phase 11.4 — The Metroid Prime fixture set: `src/fixtures/mp/`

Five files per redesign 15's table: `gadgets.ts` (~22 majors incl. `derivedFrom` beam combos, a
resource-pool entry with progressive capacity, progressive tanks, the 12-copy facet-less
artifact), `puzzles.ts` (multi-condition bosses, capability-gated boss, the world-scope shrine →
final-boss chain with `guarantee` bounded by world length), `reaches.ts` (7-Reach
`WorldLengthPolicy {7,7}` + per-Reach `AreaCountConfig` overrides flexing 1–4, one Reach with 3
Areas × up to ~21 Spaces), `portals.ts` (bidirectional hubs + a one-way prologue), `biomes.ts`
(5 packs). This is **test data** — structure over fidelity; keep hull params modest.

## Phase 11.5 — Tests (`src/presets.test.ts`, `src/fixtures/mp.test.ts`)

- All three presets + the MP fixture pass `defineRegistry` (no orphan tags — this will shake out
  vocabulary mistakes; fix the data, not the validator).
- **Soak matrix**: 200 seeds × each preset (geometry off): solvable + autosolve-complete every
  seed. One seed × each preset with `geometry: true` at small dims: finish succeeds, budgets
  respected, crawler normals all 90°-quantized, classic all 5°-quantized.
- Preset character (distributional, 100 seeds each): crawler has zero outdoor Spaces and zero
  landmarks; prime has ≥1 outdoor Space and 2 landmarks per Reach on average; crawler mean
  `zSpread` < classic < prime.
- MP fixture: the world-scope collectathon resolves by Reach 6 in every one of 50 seeds; the
  64-room-scale Reach generates without relaxation storms (< 3 relaxations); the one-way
  prologue portal is `oneWay: true`.

## Definition of Done

- [ ] The soak matrix is the new baseline (`pnpm test` runs it; total examples suite < ~60 s).
- [ ] The fidelity-spectrum character tests pass — presets demonstrably differ along the
      intended axes.
- [ ] `@cyclevania/examples` imports **only** the core public API (`@cyclevania/core` root
      import — no deep paths).
- [ ] Full root suite + typecheck green. STOP — hand off. Do not commit.

## Pitfalls

- Preset data must not share mutable objects across presets (freeze or factory-per-call —
  `defineRegistry` fingerprinting assumes stable input).
- Keep MP hull/geometry params small — this fixture exists for graph/schema stress, not meshing
  stress (geometry runs on it only in M14's benchmark).
