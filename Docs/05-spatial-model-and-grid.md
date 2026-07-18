# 05 · Spatial model & the grid

The grid is **logical, not geometric** — it does not force boxy, perfectly-adjacent rooms with 90°
entrances. It's a container hierarchy over a subdividable grid, and connectors can be any geometry (curved
halls, 45° tunnels, open seams, vertical shafts, snake-like runs), snapped to the PS2 palette by default.

```
Reach ─▶ Area (coarse node-grid: room-nodes in XYZ) ─▶ Room (fine subdivided cells) ─▶ Cell (a kit piece + metadata + sockets + contents)
```

## Two grids

- **Area grid (coarse)** — resolution `areaCellSize`; room-nodes sit in area space, joined by connectors
  (not orthogonally packed).
- **Room subdivided grid (fine)** — resolution `roomCellSize`; the **AreaComposer** sets each room's
  **extent envelope**, and the **RoomComposer** lays out the cells within it, assigning one kit piece per
  cell from the GeometryKit (via `classifyCell` → surface role → `piecesForRole`, snapped by `snapAngle`).

## Sockets (not always doors)

A `Socket` is a 3D directional aperture on a cell face: `kind` (`arch|cave-mouth|open|threshold|
crawlspace|climb-face|drop-hatch|vent`), a `traversal` (`walk|climb|crawl|drop|open|ladder|rope|vertical`),
an optional `gate` (a `Rule`), and `oneWay`. A Morph-Ball-style crawlspace is
`{ kind:"crawlspace", traversal:"crawl", gate: have("small-form") }`.

## Descriptors (what you render)

CycleVania emits **kit-piece assignments per cell + socket transforms + connector plans** — never meshes:

```
ReachDescriptor { areas, links, startAreaId, bounds }
  AreaDescriptor { areaId, regionId, role, nodeGrid, rooms, connectors, portals, gadgets, bounds, styleId }
    RoomDescriptor { nodeId, role, kind, origin, footprint, cells, sockets, bounds, styleId }
      CellDescriptor { coord, role, kitId, yaw, traversal?, sockets, contents }
    ConnectorPlan { fromSocket, toSocket, kind, cells, requires?, requiredCaps? }
    PortalSpec { key, edge?, trigger, spawn, spawnYaw, requires?, requiredCaps? }
```

Your realizer iterates `cells`, dispatches by `kitId`/`role` to your mesh builders, hooks `colliders` and
`portals` into physics, and places `gadgets`/`contents`. `requiredCaps` always carries the **full** gate
cap-set (never collapsed to one "primary").

## Phase A vs Phase B

Phase A ships a hollow-box room realization (rooms in a row + straight connectors) to prove the two-tier
grid + kit assignment end-to-end. Phase B expands the AreaComposer to a cyclic room-graph with
curved/45°/open/vertical/snake connectors, adjacency-grammar (optionally WFC) per-cell selection, and the
grow-into-leftover-space pass. The descriptor shape is already Z/loop-ready.
