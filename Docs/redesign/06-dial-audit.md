# 06 · Dial audit — every host-facing knob in one place

> A consolidated reference of every configurable dial across `Docs/redesign/`, plus a handful this
> revision adds because they were genuine gaps, not because the brief asked for them directly.
> Nothing here is final — the point is to have the whole surface visible in one place *before*
> committing to an implementation plan, per your last question.

## Consolidated table

| Dial | Owner | Shape | Doc |
|---|---|---|---|
| `WorldSeed` | World | opaque seed | [01](./01-mission-graph.md) |
| `MaxReachCount` | World | optional integer ceiling (unbounded Worlds) | [05](./05-determinism-and-extensibility.md) |
| `WorldLengthPolicy` (`min`, `max`, `weights?`) | World | ranged, weighted — drawn once as `L` | [02](./02-composers-and-complexity.md) |
| `BaseCeiling`, `K_MUL`, `K_ADD`, `TIER_SIZE` | `ComplexityConfig` | curve shape constants | [02](./02-composers-and-complexity.md) |
| `JITTER_FRAC`, `LOOKBEHIND_PULL` | `ComplexityConfig` | entropy/anchoring strength | [02](./02-composers-and-complexity.md) |
| `MIN_CEILING`, `HARD_MAX`, `ABSOLUTE_HARD_MAX` | `ComplexityConfig` | clamp bounds | [02](./02-composers-and-complexity.md) |
| `AreaCountConfig` (`min`, `max`, `weights?`) | `ReachComposer` | ranged, weighted, default `{5,5}` | [02](./02-composers-and-complexity.md) |
| `ReachModifierPolicy.poolAt/requiredRange` | modifiers | depth-curved pool + min/max | [02](./02-composers-and-complexity.md) |
| `DialPatch` (`complexity`, `gadgetEconomy`, `puzzleEconomy`, `reward`, `hazard`, `structure`, `custom`) | per-modifier | additive/multiplicative deltas | [02](./02-composers-and-complexity.md) |
| `ReachRequest.gadgetEconomyOverride`/`.puzzleEconomyOverride`/`.template` | per-request | host override at request time | [02](./02-composers-and-complexity.md) |
| `GadgetEconomyConfig.min/max` | progression items | per-Reach count range | [03](./03-locks-keys-and-gadgets.md) |
| `CapabilityDef.powerWeight(level)` | scheduler | 0..1 curve | [03](./03-locks-keys-and-gadgets.md) |
| `CapabilityDef.guarantee.withinReachLevels` | scheduler | pity-window bound | [03](./03-locks-keys-and-gadgets.md) |
| `PuzzleEconomyConfig.min/max` | puzzles | per-Reach count range, independent of Gadgets | [07](./07-puzzles-and-challenges.md) |
| `PuzzleDef.powerWeight(level)` / `.guarantee` | puzzle scheduler | identical shape to Capabilities | [07](./07-puzzles-and-challenges.md) |
| `PuzzleDef.scope` (`room\|area\|reach\|world`) | puzzle authoring | how far a condition may draw from | [07](./07-puzzles-and-challenges.md) |
| `PuzzleDef.class` (`required\|optional-reward\|optional-shortcut\|cosmetic`) | puzzle authoring | gates progress or not | [07](./07-puzzles-and-challenges.md) |
| `ReachTemplate` (`criticalPath`, `nodes.slots`, `branches`, `gating`, `loops`) | template | structural shape | [01](./01-mission-graph.md) |
| `SpaceBudget` (voxel volume, poly allowance) | Space | per-Space ceiling | [04](./04-spatial-composition-and-sockets.md) |
| connectivity-degree `{min,max}` / `maxDegree` per Space `kind` | Space | integer range | [04](./04-spatial-composition-and-sockets.md) |
| `ContentAnchorKind` (`minSeparation`, `clearanceFromStructural`, `targetDensity`, `allowedSurfaces`) | content Sockets | per-kind placement dials | [04](./04-spatial-composition-and-sockets.md) |
| `BiomePack` (palette, materials, hazard set, dressing set, noise) + sub-biome blend weights | biome | content pack | [04](./04-spatial-composition-and-sockets.md) |
| landmark count per Reach (1–2) + visibility bias | landmarks | count + placement heuristic | [04](./04-spatial-composition-and-sockets.md) |

## Default calibration philosophy

A dial's *shape* (min/max/weights, a curve, a bucket) is CycleVania's concern; the *shipped
default values* inside that shape are a separate, softer question — and it's worth having a
stated target rather than picking round numbers arbitrarily. A useful, well-known reference scale
for calibrating those defaults: aim for a single Reach at roughly 1/5th the spatial size and
playtime of a full, well-known 3D metroidvania world (Metroid Prime's is a reasonable, widely
understood benchmark), with a full World spanning `WorldLengthPolicy`'s default range of Reaches —
so an average World lands at, or just above/below, that same full-game scale. Concretely, this
means:

- `AreaCountConfig` default `{ min: 5, max: 5 }` and `WorldLengthPolicy` default `{ min: 4, max: 6 }`
  are chosen together, not independently — 5 Areas × ~5 Reaches is the "one reference-scale world"
  target, and the 4–6 range is what keeps individual Worlds from being mechanically identical in
  length while staying centered on that target.
- `ComplexityConfig`'s `BaseCeiling` and related curve constants should be tuned so that **one**
  Reach's aggregate footprint/room-count/traversal budget approximates 1/5th of that reference
  scale — this is a tuning target for whoever picks the actual numbers, not a formula CycleVania can
  derive on its own, since "spatial size" only becomes a concrete unit once a host's voxel/world-grid
  scale is fixed (see the units-conversion note on `MagnitudeFacet` in [03](./03-locks-keys-and-gadgets.md)).
- This is a **default calibration target, not a constraint** — every dial above remains fully
  host-overridable; a host wanting a much shorter or much larger reference scale changes exactly the
  same knobs, just aimed at a different target number.

## Proposed additions worth thinking about now

None of these came from the brief directly — they're gaps I noticed while auditing the rest.

- **`ReachTemplatePool`, not a single `templateFor(reachIndex)`.** Right now [01](./01-mission-graph.md)
  talks about "the World's default `templateFor(reachIndex)`" as if it's one deterministic function.
  For real variety it should follow the same pattern already established for
  `ReachModifierPolicy` — a depth-scoped, weighted **pool** of templates, drawn from with a seeded
  weighted choice: `ReachTemplatePool.poolAt(depth): { template: ReachTemplate; weight: number }[]`.
  Otherwise every Reach at a given depth ends up with the same macro-shape, which undercuts the
  "every run feels non-repeating" goal as much as a boxy room would.
- **A baseline hazard/reward curve, independent of modifiers.** `dials.hazard.densityMul` and
  `dials.reward.lootTierBonus` ([02](./02-composers-and-complexity.md)) currently only exist as
  *modifier deltas* — there's no ambient, depth-driven baseline they're adjusting *from*, unlike
  complexity's `ComplexityConfig` curve. Worth adding `HazardBaselineConfig`/`RewardBaselineConfig`
  (same curve shape as `ComplexityConfig`) so a world still feels like it's escalating even for a
  player who never picks a single Reach modifier.
- **Loop-closure density, not just a boolean.** `loops.guaranteeAtLeastOne` ([01](./01-mission-graph.md))
  is a floor, not a dial — there's currently no way to ask for *more* backtracking-reward loops
  beyond the guaranteed minimum. A `loops.extraAttempts` or `loops.density` (0..1, biasing how many
  additional shortcut-closure attempts `AreaComposer` makes beyond the guarantee) directly serves
  the brief's "amount of backtracking required" as a tunable, not just a pass/fail guarantee.
- **Secret/optional-content fraction.** Nothing currently distinguishes "on the critical path" from
  "deliberately optional bonus content" at a *tunable rate* — `vault` branches exist structurally
  ([01](./01-mission-graph.md)) but there's no dial for *how many* Locations per Area should be
  off-critical-path secrets versus required progress. A per-Area `secretFraction` would let a host
  dial a world from "linear and lean" to "stuffed with side content" without touching the template.
- **Socket signature strictness (exact match vs. fuzzy compatibility).** [04](./04-spatial-composition-and-sockets.md)'s
  `signature` matching is currently exact-string. A `signatureFuzziness` dial (accept a compatibility
  *score* above a threshold instead of exact equality) would let hosts trade "always geometrically
  safe" for "more visually varied connections," as a spectrum rather than an all-or-nothing choice.
- **Connector length bounds.** [04](./04-spatial-composition-and-sockets.md) never bounds how far
  apart two Sockets can be before `ConnectorComposer` grows a hull between them — a `minLength`/
  `maxLength` per connector `traversal` kind (a `crawl` connector probably shouldn't span as far as
  a `walk` one) directly controls how sprawling versus compact an Area feels.
- **Per-Area biome-swap chance, distinct from within-Area gradient blending.** [04](./04-spatial-composition-and-sockets.md)'s
  sub-biome blending handles *smooth* transitions within one Area; there's no separate dial for
  "how often does the *next* Area in this Reach switch to a visually distinct biome outright" (a
  harder, Area-granularity cut, as opposed to the soft gradient) — both are legitimate, different
  tools, and only one currently exists.
- **`GeometryKit.polyBudgetPerArea` / max unique kit pieces.** [04](./04-spatial-composition-and-sockets.md)'s
  `SpaceBudget` bounds one Space's own volume/poly allowance, but there's no Area-wide ceiling on
  total unique kit pieces or aggregate polycount — worth adding before a target-platform memory
  budget becomes an implementation-time surprise rather than a documented dial.

None of these are structural changes to anything already agreed — they're all extra knobs that slot
into the existing budget-cascade/registry pattern the rest of these docs already establish, which is
exactly why they're easy to add later without a redesign if you'd rather defer them.
