# 14 · Dial reference & presets

> Every host-facing dial across the whole design, in one table — the complete tuning surface.
> Anything that varies output and isn't here is a bug (either an undocumented dial or a buried
> constant, both forbidden). Then: the calibration philosophy behind the shipped defaults, and the
> three preset datasets that demonstrate the fidelity spectrum.

## The consolidated dial table

| Dial | Owner / scope | Shape | Doc |
|---|---|---|---|
| `WorldSeed` | World | opaque string | [02](./02-determinism.md) |
| `WorldLengthPolicy` | World | `{min, max?, weights?}` — drawn once as `L` | [04](./04-worlds-reaches-and-pacing.md) |
| `ComplexityConfig`: `BaseCeiling`, `K_MUL`, `K_ADD`, `TIER_SIZE`, `JITTER_FRAC`, `LOOKBEHIND_PULL`, `MIN_CEILING`, `HARD_MAX`, `ABSOLUTE_HARD_MAX` | World | curve + clamp constants | [04](./04-worlds-reaches-and-pacing.md) |
| `HazardBaselineConfig` / `RewardBaselineConfig` | World | tier-curve shapes | [04](./04-worlds-reaches-and-pacing.md) |
| `ReachTemplatePool.poolAt(depth)` | World | weighted template pool per depth | [03](./03-mission-graph.md) |
| `ReachModifierCatalog` (`riskWeight`, `rewardWeight`, `minDepth`, `dials`, `excludesTags`) | modifiers | per-modifier `DialPatch` | [04](./04-worlds-reaches-and-pacing.md) |
| `ReachModifierPolicy.poolAt` / `.requiredRange` | modifiers | depth-scoped pool + optional→mandatory ramp | [04](./04-worlds-reaches-and-pacing.md) |
| `ReachRequest` overrides (`template`, `gadgetEconomyOverride`, `puzzleEconomyOverride`) | per request | host levers at request time | [04](./04-worlds-reaches-and-pacing.md) |
| `AreaCountConfig` | Reach | `{min, max, weights?}` | [04](./04-worlds-reaches-and-pacing.md) |
| `GadgetEconomyConfig` | Reach | `{min, max}` progression items | [05](./05-capabilities-and-facets.md) |
| `CapabilityDef.powerWeight(level)` / `.guarantee.withinReachLevels` | per capability | 0..1 curve · pity bound | [05](./05-capabilities-and-facets.md) |
| scheduler constants `MAX_LEVEL_SHIFT`, `SOFTNESS` | schedulers | logistic-curve shape | [05](./05-capabilities-and-facets.md) |
| `PuzzleEconomyConfig` | Reach | `{min, max}` puzzles | [06](./06-puzzles-locks-and-recipes.md) |
| `PuzzleDef.powerWeight` / `.guarantee` / `.scope` / `.class` | per puzzle | identical shapes to capabilities | [06](./06-puzzles-locks-and-recipes.md) |
| `LockPacingConfig` (`teachTestCombine`, `combineChance`) | Reach | pacing bias | [06](./06-puzzles-locks-and-recipes.md) |
| `PlacementWeightConfig` (`entrySpaceWeight`, `depthExponent`, `vaultBonus`, `behindGateBonus`, `perRegionCap`, `sphereSpreadBonus`) | fill | placement weights | [03](./03-mission-graph.md) |
| `ReachTemplate.gating` (`lockFraction`, `compoundChance`, `keepEntryOpen/ExitOpen`) | template | structural gating | [03](./03-mission-graph.md) |
| `ReachTemplate.loops` (`guaranteeAtLeastOne`, `density`) | template | loop guarantee + extra-loop dial | [03](./03-mission-graph.md) |
| `secretFraction` | Area | 0..1 off-path secret rate | [07](./07-spatial-skeleton.md) |
| connectivity-degree table (per Space kind/role) | Area | `{min, max}` per kind | [07](./07-spatial-skeleton.md) |
| `outdoorChance` · `largeSpaceChance` · landmarks per Reach | Area/Reach | seeded chances + count | [07](./07-spatial-skeleton.md) |
| z-derivation constants (`zSpread` base + `K·sqrt` diminishing-returns shape) | Area | verticality response | [07](./07-spatial-skeleton.md) |
| `SignatureConfig.fuzziness` (+ custom `compatible`) | sockets | 0 (exact) .. 1 (loose) | [07](./07-spatial-skeleton.md) |
| connector length bounds (per traversal) · kind weights | connectors | `{min, max}` + weights | [07](./07-spatial-skeleton.md) |
| force-layout constants (`K_SPRING`, `K_REPEL`, `zSeparation`, iterations, `DAMPING`, `MAX_STEP`) | layout | simulation shape | [07](./07-spatial-skeleton.md) |
| `HullArchetypeDef` set (sdf, sizeRange, noise, kinds/roles/biomes, landmark, weight) | hulls | the shape library | [08](./08-volumetric-composition.md) |
| `BiomePack` set (palette, materials, noise, hazards, dressing, waterLevel, outdoorAffinity, seamK, subBiomes) | biomes | content packs | [08](./08-volumetric-composition.md) |
| `BiomePlanConfig` (`biomesPerReach`, `areaSwapChance`, `gradientBlend`) | biomes | transition strategy | [08](./08-volumetric-composition.md) |
| `ContentAnchorKind` set (`allowedSurfaces`, `minSeparation`, `clearanceFromStructural`, `targetDensity`) | anchors | per-kind scatter dials | [08](./08-volumetric-composition.md) |
| `SpatialRecipeDef` set | recipes | lock → space shapes | [06](./06-puzzles-locks-and-recipes.md) |
| `FidelityProfile` (`angleStepDeg`, `voxelRes`, `maxDim`, `snapNormals`) | finish | the spectrum knob | [09](./09-naturalization-and-kit.md) |
| kit `cellSize` multiplier · `polyBudgetPerArea` · `maxUniquePieces` | finish | modularity + memory ceilings | [09](./09-naturalization-and-kit.md) |
| dressing densities (per kind, per biome) | finish | scatter weights | [09](./09-naturalization-and-kit.md) |
| `HorizonPolicy` (`ahead`, `requestFor`, `evictBehind`) | orchestration | prefetch behavior | [12](./12-orchestration-and-host-integration.md) |
| `geometry` (per generation call) | orchestration | run/skip the finish pass | [01](./01-architecture.md) |

## Calibration philosophy

A dial's *shape* is CycleVania's concern; the shipped *default values* target a stated reference
scale rather than round numbers: **one Reach ≈ 1/5th the spatial size and playtime of a full,
well-known 3D metroidvania world** (Metroid Prime's is the benchmark), with
`WorldLengthPolicy {min: 4, max: 6}` so an average World lands at or just past full-game scale.
Concretely: `AreaCountConfig {5,5}` × ~5 Reaches is the reference target;
`ComplexityConfig.BaseCeiling` and friends are tuned so one Reach's aggregate footprint approaches
1/5 of that scale — a tuning target, not a formula, since "spatial size" only becomes concrete
once a host fixes its unit-per-voxel scale. Every default remains fully overridable; a host
wanting a different scale moves the same knobs at a different target.

Genre reference points used for economy defaults (grounding, not prescription): a handful of
required lock beats per major dungeon, concentrated in capstones; item/reward spacing even rather
than clustered; puzzles denser early (teaching), gadget power later (the schedulers' logistic
shapes encode exactly this).

## The three shipped presets (`@cyclevania/examples`)

Complete, loadable datasets — each a reproduction-bundle-ready registry proving one point on the
fidelity spectrum. They share the same example capability/puzzle catalogs where possible so diffs
between presets isolate the *spatial* dials.

### `crawler` — the boxy, orthogonal, lateral dungeon crawler

- `FidelityProfile { angleStepDeg: 90, voxelRes: coarse }`; hull archetypes: `box-hall`,
  `box-room` only; noise amplitude 0; `outdoorChance` 0; landmarks 0; z-buckets near-zero
  (lateral world); degree table tight (2–3); `AreaCountConfig {3,4}`; small budgets.
- Proves: the same pipeline produces clean, classic, grid-feeling dungeons — and everything
  (solvability, sim, kit, collision, Inspector) works identically at this end.

### `classic` — the default balanced metroidvania

- `angleStepDeg: 5` (the flagship PS2-era faceted look), moderate noise, occasional large/outdoor
  Spaces, 1 landmark per Reach, the full default dial set from this doc's table, the 24-verb
  example gadget catalog + the lock-taxonomy example puzzles.
- Proves: the shipped defaults are a good game out of the box; this is the preset the Inspector
  opens with and most docs' examples assume.

### `prime` — the epic-scale showcase

- `AreaCountConfig` flexing per Reach (up to Metroid Prime-like 3 Areas × 10–21 Spaces), 2
  landmarks + vistas, outdoor bowls with water levels, sub-biome gradients + area swaps, deep
  z-budgets with drop-loops, a world-scope collectathon puzzle chain, modifier catalog with a
  mandatory late-game ramp.
- Proves: scale, verticality, memorability, and the world-scope puzzle machinery — and doubles as
  the performance benchmark dataset.

The separate **Metroid Prime reconstruction fixture set** ([15](./15-verification-and-test-strategy.md))
is *test data* (encoding a real shipped game's structure to stress the schema), distinct from the
`prime` *preset* (an original, generative dataset at that scale).
