# 00 · Overview

CycleVania generalizes a proven metroidvania procgen pipeline into a reusable library. The importing
game focuses on **content as data**; CycleVania owns the **algorithms**.

## The pipeline

1. **Grammar** (`template/`) — a data-driven `ReachTemplate` becomes a `RegionGraph` (regions, gated
   edges, placeable locations).
2. **Fill** (`fill/`) — `assumedFill` places progression items so the world is solvable **by
   construction**; an independent `isSolvable` check verifies it.
3. **Spatial realization** (`composers/` + `spatial/` + `descriptors/`) — each region becomes an **Area**
   (a grid of room-nodes); each room a **subdivided cluster of cells**; each cell a **kit-piece
   assignment** chosen from the game's GeometryKit. Output is abstract descriptors, not meshes.
4. **Simulate** (`sim/`) — a deterministic playtest reducer walks the result like a player (the inspector
   REPL and the "bot completes a Reach" test both use it).

## Composer hierarchy

- **WorldComposer** — the `WorldSeed` + all Reaches (≥1); cross-Reach progression (carry caps forward).
- **ReachComposer** — all Areas of a Reach; each Area's extent, depth, biome palette; wires portals from
  region edges (carrying their gates).
- **AreaComposer** — the "meat": room count, per-room **extent envelopes**, room kinds, portals, biome.
- **RoomComposer** — a room's cells/contents **within the envelope it's handed** (kit assignment, sockets,
  content anchors).

## Reading order

- [01 · Determinism law](./01-determinism-law.md)
- [02 · Logic & solvability](./02-logic-and-solvability.md)
- [03 · Reach templates](./03-reach-templates.md)
- [04 · Registries & the GeometryKit](./04-registries-and-geometrykit.md)
- [05 · Spatial model & the grid](./05-spatial-model-and-grid.md)
- [09 · Gadget & lock cookbook](./09-gadget-and-lock-cookbook.md)

## Status

Phase A (vertical slice) is complete: determinism core, logic/graph/fill, template DSL, registries,
spatial primitives + minimal Area/Room composers producing a subdivided cell grid, the simulator, the
24-gadget examples pack, and a minimal inspector. Phase B expands the spatial realization (cyclic
room-graphs, curved/45°/vertical/snake connectors, adjacency grammar/WFC, async orchestration + horizon).
