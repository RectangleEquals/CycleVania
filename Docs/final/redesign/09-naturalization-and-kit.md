# 09 · Naturalization & the generated kit (L4 — real geometry)

> The finish pass converts an Area's composed field into engine-agnostic triangle geometry — split
> into a **deduplicated, world-grid-aligned modular kit** plus placement instances — alongside a
> serializable **occupancy/collision grid** and **dressing anchors**. L4 is strictly a finishing
> pass: it converts, classifies, and exports; it never makes a structural decision. Swapping
> fidelity targets never touches solvability, layout, gating, or anchors.

## Fidelity profiles — the spectrum as one config

```ts
interface FidelityProfile {
  angleStepDeg: number | null;   // normal quantization grid: 90 (orthogonal) · 45 (chamfer) ·
                                 // 15 (chunky retro) · 5 (PS2-era faceting, the flagship default) ·
                                 // null (no snap — smooth organic)
  voxelRes: number;              // world units per meshing cell (coarse = chunky, fine = detailed)
  maxDim: number;                // per-Area voxel-grid clamp; res is raised (coarsened) to fit (default 72)
  snapNormals: boolean;          // false only with angleStepDeg null
}
```

This single config, together with hull-archetype and noise choices ([08](./08-volumetric-composition.md)),
*is* the fidelity spectrum from [00](./00-goals-and-principles.md): `90° + box hulls + zero noise`
is a clean orthogonal dungeon crawler; `5° + organic hulls + fbm` is the PS2-look organic world;
`null` is smooth-modern. One pipeline; data decides.

### Normal quantization

Snap a unit normal to the nearest orientation on the angular grid, in spherical coordinates,
using deterministic trig ([02](./02-determinism.md)):

```
STEP        = angleStepDeg · π/180
azimuth     = datan2(n.y, n.x)            → round(azimuth / STEP) · STEP
elevation   = datan2(n.z, |n.xy|)         → round(elevation / STEP) · STEP
n′          = (dcos(el)·dcos(az), dcos(el)·dsin(az), dsin(el))
```

Quantizing the **hermite normals before vertex placement** (not the output mesh after) is what
produces coherent faceting: nearby cells with similar true normals snap to the *same* plane, their
QEF vertices co-planarize, and large clean facets emerge — the hand-modeled retro look, not a
decimation artifact. It's also what makes kit deduplication effective (identical local surfaces ⇒
identical pieces).

## Dual contouring (the mesher)

Chosen over marching cubes because DC represents **sharp features exactly** (one QEF-placed vertex
per cell can sit on an edge or corner), which the orthogonal end of the spectrum requires and the
faceted middle benefits from.

```
inputs: field, origin, dims (cells), res, fidelity
data:   CORNERS[8] (fixed order), EDGES[12] (fixed corner pairs)

pass 1 — vertices:
  for each cell (x → y → z fixed order):
    sample field at 8 corners
    if no sign change: skip
    for each of the 12 edges with a sign change:
      t     = d₀ / (d₀ − d₁)                       # linear zero crossing
      p     = lerp(corner₀, corner₁, t)
      n     = sdfNormal(field, p); if snap: quantize(n)
      accumulate (p, n) into the cell's QEF
    v = solveQEF(points, normals)                   # 3×3 normal equations; fixed-order accumulation;
                                                    # singular ⇒ mass point; always clamped into the cell
    vNormal = quantize(−Σnᵢ)                        # faces the OPEN interior (negative side)
    store cellVertex[x,y,z] = (v, vNormal)

pass 2 — quads:
  for each axis a ∈ {x, y, z}, for each grid edge along a with a sign change:
    the 4 cells sharing that edge each own a vertex → emit a quad (two triangles)
    winding: from the solid side toward the open side (consistent outward = into open space)
output: Mesh { positions: number[], normals: number[], indices: number[] }
```

Determinism notes ([02](./02-determinism.md)): fixed iteration orders, fixed corner/edge tables,
insertion-ordered vertex map, no floating accumulation outside fixed order. Meshing runs per Area
over the L3 voxelization extents; `maxDim` guards worst-case cost by coarsening `res`, recorded in
Area meta when it fires.

## The generated kit — partition, classify, dedup

The mesh is split into modular, world-grid-aligned pieces so hosts get instanced, reusable,
memory-bounded geometry instead of one monolithic mesh:

```ts
interface PieceMeta {
  surface: "floor" | "wall" | "ceiling" | "slope" | "overhang" | "mixed";  // same cones as 08
  biome: string;
  materialHint: string;            // from BiomePack.materials[surface]
  traversal?: Traversal;           // when the piece realizes a socket aperture
  collider: "solid" | "none" | "trigger";
  tags: string[];                  // e.g. "revealable" (perception locks), "sky-open", recipe tags
}
interface GeneratedPiece { id: string; positions: number[]; normals: number[]; indices: number[]; meta: PieceMeta; }
interface PieceInstance  { coord: [number, number, number]; pieceId: string; yaw: number; }
interface GeneratedKit   { cellSize: number; pieces: GeneratedPiece[]; }
```

Algorithm:

1. **Partition** triangles into kit cells (`cellSize` = an integer multiple of `voxelRes`,
   default 4×) by centroid.
2. **Localize**: re-express each cell's triangles in cell-local coordinates; **round to 1e-3**
   (part of the output contract — this rounding is what makes step 3 stable).
3. **Canonicalize yaw**: try the 4 cardinal rotations of the local piece; keep the
   lexicographically-smallest buffer as the canonical piece and record the rotation as the
   instance's `yaw` — so a north-facing and east-facing identical wall dedupe to one piece.
4. **Hash** the canonical buffers (FNV-1a over positions+normals+indices) → piece id (`gp` +
   ordinal of first appearance, mapping recorded).
5. **Classify** `PieceMeta.surface` from the area-weighted average normal (the cones from
   [08](./08-volumetric-composition.md)); attach biome/material/tags; apertures inherit the
   socket's traversal; pieces inside a `revealable`-tagged recipe/secret region get
   `collider: "none"` + tag `revealable` (the host toggles collision on the perception event).
6. **Emit** `GeneratedKit` (unique pieces) + `PieceInstance[]`.

Quantized fidelity makes many cells produce identical local surfaces, so
`pieces.length ≪ instances.length` — asserted in tests. Budget dials: `polyBudgetPerArea` and
`maxUniquePieces` — exceeding either is a `GenError` naming the Area and the worst offenders
(typically ⇒ the host coarsens `voxelRes` or reduces noise), never a silent decimation.

## Occupancy grid & collision

```ts
interface OccupancyGrid { origin: Vec3; res: number; dims: [number, number, number]; solid: Uint8Array; }
// serialized as OccupancyData with solid: number[] (10)
```

- Derived per Area from the field's sign per voxel (1 = solid). Out-of-bounds queries return
  **solid** (the world edge is a wall). Sky-open columns of outdoor Spaces are open above the
  terrain up to the envelope top.
- **`collideSphere(grid, pos, r): Vec3`** — the shared collision primitive (Inspector Play mode
  *and* host games): up to 4 iterations of sphere-vs-cell-AABB resolution over the cells the
  sphere overlaps; each iteration pushes out along the least-penetration axis; a center inside a
  solid cell escapes along the least-penetration axis of that cell first. Deterministic, cheap,
  serializable — a host that wants fancier physics can still feed the same grid to its engine.
- The grid is also the navigation substrate for host bots/pathfinding (walkable = open cell with
  solid support below, exported helper `isWalkable(grid, coord)`).

## Dressing anchors

Bulk aesthetic scatter — distinct from L3 content anchors (gameplay-relevant, manifest-bound):
dressing is dense, cheap, purely decorative, and generated here because it depends on final
surfaces:

```ts
interface DressingAnchor { pos: Vec3; kind: string; up: Vec3; }  // kinds from BiomePack.dressing
```

Per Area, from `${areaRoot}:dress`: iterate surface cells; a seeded fbm threshold per dressing
kind decides placement (stalactites on ceilings/overhangs, foliage on floors near sky-open
columns, rubble near walls, water surfaces at `min(biome.waterLevel, column low point)`); density
scaled by `BiomePack.dressing` weights and the hazard/reward baselines. Anchors only — the host
realizer decides what a "stalactite" looks like. Dressing respects content-anchor clearances (it
runs last and rejects within `minSeparation` of accepted L3 anchors).

## What the finish pass explicitly does not do

- Decide anything: no outdoor flags, no socket moves, no content changes — inputs are final.
- Textures, materials, lighting: `materialHint`/palette data pass through; realization is the
  host's.
- LOD: single-LOD output by design; `voxelRes`/`cellSize` are the knobs. (A host can decimate
  pieces engine-side; CycleVania stays out of it.)
- Physics beyond the grid: the occupancy grid + piece `collider` flags are the whole contract.
