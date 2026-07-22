# Dev Guide 00 — The generation pipeline (read this first)

> Audience: a developer new to CycleVania. This explains **how a seed becomes a
> guaranteed-solvable, organically-shaped, engine-agnostic 3D level with real geometry**,
> layer by layer, with the algorithm behind each step. No graphics/experience assumed.

CycleVania turns a **seed + game data** (items, locks, hull archetypes, biomes) into a
**World** of Reaches → Areas → Rooms, and — in Phase D — into **actual triangle geometry**
split into a reusable, grid-aligned **kit** plus a **collision grid**. The game textures and
renders it; CycleVania never imports an engine, the DOM, or Node.

## The 4-layer pipeline (Gemini model)

```
  seed + registry
        │
        ▼
┌──────────────────────────────────────────────────────────────────────┐
│ L1  MISSION GRAPH        template/ + graph/ + fill/                     │
│     region graph · lock/key grammar · assumed-fill · spheres           │
│     → a provably solvable ABSTRACT graph (who connects to whom, gated  │
│       by which capability). NO space yet.                              │
└──────────────────────────────────────────────────────────────────────┘
        │  regions, edges (gates), item placement
        ▼
┌──────────────────────────────────────────────────────────────────────┐
│ L2  SPATIAL TOPOLOGY     composers/ (+ layout/forcedirect)             │
│     lay regions out in 3D · branching walk / force-directed · cyclic  │
│     shortcuts · portals · per-room extent envelopes                    │
│     → every room has an origin + bounds + sockets. Still "boxes".      │
└──────────────────────────────────────────────────────────────────────┘
        │  rooms (origin, bounds, sockets), connectors
        ▼
┌──────────────────────────────────────────────────────────────────────┐
│ L3  VOLUMETRIC LAYOUT    volume/ (sdf, hulls, spline, field)           │
│     one organic SDF hull per room (biome-picked) · spline-tube        │
│     corridors · compose an area SIGNED DISTANCE FIELD                  │
│     → field(p) < 0 inside walkable space, > 0 in solid rock.          │
└──────────────────────────────────────────────────────────────────────┘
        │  area field : (x,y,z) → distance
        ▼
┌──────────────────────────────────────────────────────────────────────┐
│ L4  NATURALIZE → GEOMETRY  geometry/ (fidelity, mesher, kit, ...)      │
│     dual-contour the field · snap normals to the 5° grid · split the  │
│     mesh into dedup'd grid-aligned kit pieces · occupancy grid ·      │
│     dressing anchors                                                   │
│     → AreaDescriptor.kit + .instances + .occupancy + .dressing         │
└──────────────────────────────────────────────────────────────────────┘
```

**Why 4 layers?** Solving progression, space, and aesthetics *at once* is intractable and
produces boxy, samey levels. Separating them lets each layer use the right algorithm:
graph grammar for logic, force/branch layout for space, SDFs for organic shape, dual
contouring for faceted geometry. Solvability is fixed forever in L1 and never touched again.

## Where each layer lives in code

| Layer | Package path | Key files |
|---|---|---|
| L1 Mission | `core/src/{template,graph,fill,logic}` | `grammar.ts`, `region-graph.ts`, `assumed-fill.ts` |
| L2 Topology | `core/src/composers` + `core/src/layout` | `area-composer.ts`, `reach-composer.ts`, `forcedirect.ts` |
| L3 Volume | `core/src/volume` | `sdf.ts`, `hulls.ts`, `spline.ts`, `field.ts` |
| L4 Geometry | `core/src/geometry` | `fidelity.ts`, `mesher.ts`, `kit.ts`, `collision.ts`, `dress.ts` |

## The one-call path

```ts
import { composeWorld } from "@cyclevania/core";
import { demoRegistry, demoTemplate } from "@cyclevania/examples";

const world = composeWorld(
  { registry: demoRegistry(), seed: "expedition-7" },
  { reachCount: 3, template: demoTemplate, carryCaps: true, geometry: true }, // geometry:true runs L4
);
// world.descriptor.reaches[i].areas[j].kit / .instances / .occupancy / .dressing
```

- **`geometry` defaults OFF.** L1–L3 always run (fast); L4 (voxel dual contouring) is heavy,
  so bulk solvability soaks skip it. The inspector and real games pass `geometry: true`.
- Every layer is **deterministic**: same seed → byte-identical output (see `01-determinism`).

## What comes out (the output contract)

`AreaDescriptor` (per area) now carries, in addition to the abstract `rooms`/`connectors`:

| Field | Meaning |
|---|---|
| `kit: GeneratedKit` | `{ cellSize, pieces[] }` — **unique** local meshes (dedup'd), each `{ id, positions[], normals[], indices[], meta }` in cell-local coords |
| `instances: PieceInstance[]` | `{ coord, pieceId, yaw }` — where to place each kit piece on the world grid |
| `occupancy: OccupancyData` | `{ origin, res, dims, solid[] }` — 1 = solid rock; the collision grid |
| `dressing: DressingAnchor[]` | `{ pos, kind, up }` — water/stalactite/foliage/rubble anchors |

The abstract `RoomDescriptor.cells` / `ConnectorPlan.cells` **stay** as a fallback + the
simulator/inspector's abstract view. Geometry is additive.

## Read next

- `01-geometry-backend.md` — the SDF → dual-contour → kit → collision algorithms in detail.
- `02-gadget-economy.md` — how gadgets are chosen/placed and how they reshape the world.
- `03-extending.md` — add a hull archetype / biome / gadget / template; run; test; screenshot.
