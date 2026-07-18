# 04 · Registries & the GeometryKit

`defineRegistry(input)` is the single data-injection surface. It validates and normalizes a game's data
into a `Registry` the composers consume.

```ts
const registry = defineRegistry({
  grid: { areaCellSize: 16, roomCellSize: 2, snap: "ps2" },   // REQUIRED: grid resolutions + snap policy
  geometryKit: myKit,                                          // REQUIRED: the surface-role grammar
  items: { catalog: [...ItemDef], startCaps: [] },
  locks: { name: RuleBuilder | ChallengeTemplate },
  rooms?: {...}, connectors?: {...}, styles?: {...},
  complexity?: ComplexityConfig,                               // defaults to DEFAULT_COMPLEXITY
});
```

## Items & the affordance contract

An `ItemDef` has an `id`, a `class` (`progression|useful|filler|bonus`), and (for progression) the
capability it `grants`. It may carry a `use` effect (what `/use` does in the simulator) and a
**`profile`** — the *affordance* half of the affordance/challenge contract: what the capability affords
the generator's space budget (`grants`: reachUp, gapSpan, descend, throughMatter, revealHidden, …) and its
in-scope `bias` (zWeight, loopWeight, enableTags). This is how the generator "knows double-jump exists" →
weights Z up and permits higher ledges. *(Consumed by the Phase B AreaComposer; carried as data today.)*

## Locks & the challenge contract

A lock is a bare `RuleBuilder` (`(r) => r.have("grapple")`) or a `ChallengeTemplate` that also names a
spatial `recipe` (`hidden-crossing`, `gap-crossing`, …) and optional `sideEffects` (a solved `shatter`
opens a drop → a new edge). `solvedBy` is resolved to a concrete `Rule` at build time and is **identical**
to the rule the solver stamps on the edge — geometry and logic can never diverge.

## GeometryKit

The parent's modular kit as a **grammar**: pieces keyed by logical surface `role` (`floor`, `wall`,
`corner`, `opening`, `ramp45`, `curved`, `climb-face`, `crawlspace`, `drop-hatch`, `open`, …) with
metadata (`snapAngles`, `traversal`, `socketCapable`, `tags`, `collider`, `adjacency`). **No geometry and
no poly/art-tier live here** — CycleVania is renderer-agnostic; the RoomComposer picks a piece id per cell
and your realizer turns ids into meshes. `piecesForRole(kit, role)` and `kitRoles(kit)` query it.

## Complexity

`complexityFor(depth, config, mods)` maps a depth to a `ComplexityBudget` (footprint, roomCount,
loopChance, roomSizeMax, mazeFactor, zSpread) via a smoothstep curve to a ceiling. Deterministic; layout
samples **around** these means so linearity gets rarer with depth but never vanishes.
