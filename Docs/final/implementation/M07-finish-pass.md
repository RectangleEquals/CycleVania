# M07 · The finish pass (L4)

**Goal**: the `finish/` module — fidelity profiles, normal-quantized dual contouring with QEF,
the generated kit (partition → canonical-yaw dedup → PieceMeta), the occupancy grid +
`collideSphere`, and dressing anchors. Strictly a pure function of L3 output + config: **no
decisions**.

**Required reading**: [redesign 09 (all)](../redesign/09-naturalization-and-kit.md);
[redesign 02 §Geometry buffer determinism](../redesign/02-determinism.md).

**Prerequisites**: M06 green.

**Porting references**: legacy `core/src/geometry/{fidelity,mesher,kit,collision,dress}.ts` —
all five are proven and close to spec; port, then apply the spec deltas called out below.

## Phase 7.1 — `finish/fidelity.ts`

`FidelityProfile` exactly per redesign 09 (`angleStepDeg: number | null`, `voxelRes`, `maxDim`
default 72, `snapNormals`). Re-export `quantizeNormal` from `math/` parameterized by
`angleStepDeg` (legacy hardcoded 5° — generalize: `STEP = angleStepDeg · π/180`; `null`/
`snapNormals: false` ⇒ identity). `isQuantized(n, stepDeg, eps)` helper for tests.

## Phase 7.2 — `finish/mesher.ts`

Dual contouring per redesign 09's pseudocode (port the legacy mesher, which matches): fixed
CORNERS[8]/EDGES[12], per-cell QEF vertex clamped to the cell, hermite normals quantized
**before** the QEF solve, cell vertex normal = quantized −Σ(edge normals), quads across
sign-changing grid edges on all 3 axes, winding solid→open. Spec deltas from legacy: take the
`FidelityProfile` (not a fixed 5°), and iterate via the shared `forEachCell` helper (M06) so
scatter and mesher agree on order.

**Structure the mesher as a resumable slab iterator** — this is the seam M10's fine-grained
async slicing needs (redesign 12: the finish pass yields every z-slab, not per Area):

```ts
export function* dualContourSteps(field, extents, profile): Generator<{ z: number; zMax: number }, Mesh>;
export function dualContour(field, extents, profile): Mesh;   // = drain the generator (the sync core)
```

The generator yields after completing each z-slab of pass 1 and each axis of pass 2; the sync
wrapper drains it to completion. Output is identical either way (assert in tests: drained vs.
stepped-with-interleaved-noops → byte-identical mesh).

## Phase 7.3 — `finish/kit.ts`

Port legacy `meshToKit`, then apply the one substantive spec delta — **canonical yaw dedup**
(redesign 09 step 3): for each cell's localized, 1e-3-rounded piece, generate its 4 cardinal
rotations, pick the lexicographically-smallest buffer as canonical, record the instance `yaw`.
PieceMeta classification per the shared surface cones; `revealable` tags flow from recipe/secret
regions (the Space spec carries the tagged region bounds — pieces whose cell centroid falls
inside get `collider: "none"` + tag). Budgets: `polyBudgetPerArea`, `maxUniquePieces` — exceeded
⇒ `GenError` listing the three largest offenders (pieceId + triangle count).

## Phase 7.4 — `finish/occupancy.ts` + `finish/collision.ts`

Port legacy occupancy + `collideSphere` (4-iteration sphere-vs-cell-AABB pushout;
out-of-bounds = solid; center-inside-solid escapes along least penetration). Add `isWalkable(grid,
coord)` (open cell with solid support below) and the serialization converter to `OccupancyData`
(`solid: number[]`). Sky-open columns: outdoor Spaces' columns open to the envelope top (the
field already encodes this — occupancy just samples it; assert in tests).

## Phase 7.5 — `finish/dress.ts`

Port legacy dressing, generalized to `BiomePack.dressing` kind/weight sets and respecting
accepted L3 anchors' `minSeparation` (dressing runs last, rejects near content anchors). Water
surfaces at `min(biome.waterLevel, column low point)`. Fork `${areaRoot}:dress`.

## Phase 7.6 — `finish/index.ts` — the single entry

```ts
export interface FinishResult { kit: GeneratedKit; instances: PieceInstance[];
                                occupancy: OccupancyGrid; dressing: DressingAnchor[];
                                stats: { tris: number; uniquePieces: number; clampedRes?: number }; }
export function finishArea(field: AreaField, spaces: SpaceSpec[], anchors: ContentAnchor[],
                           biome: BiomeResolved, fidelity: FidelityProfile,
                           budgets: GeometryBudgets, rng: Rng): FinishResult;
```

Wire into the pipeline behind the `geometry` flag: `requestReach` with `geometry: true` attaches
`FinishResult` fields to each area result. Default **off**.

## Phase 7.7 — Tests (`finish/finish.test.ts` — all fixtures ≤ 32³ cells, ≤ 5 seeds)

- **Fidelity spectrum**: a displaced-cavern fixture at `angleStepDeg: 5` — every output normal
  `isQuantized(…, 5)`; the same fixture at `90` with a box hull + zero noise — all normals
  axis-aligned **and** the mesh is exactly the box's 6 planes' worth of quads (count them); at
  `null` — normals unquantized, mesh still sane.
- **Dedup**: repeated-corridor fixture — `pieces.length < instances.length / 3`; canonical yaw:
  a fixture with the same wall facing north and east shares one pieceId with different yaws.
- **Mesh sanity**: no NaN, all indices in range, every triangle non-degenerate (area > 0).
- **Occupancy/collision**: open cell adjacent to solid stays open under `collideSphere`; a
  40-step swept walk pushed against a wall never ends inside solid; OOB solid; outdoor fixture
  has sky-open columns.
- **Budgets**: a tiny `maxUniquePieces` throws `GenError` naming offenders.
- **Determinism**: `finishArea` twice → byte-identical buffers (stable JSON compare).
- **Dressing**: anchors only on legal surfaces; none within a content anchor's separation.

## Definition of Done

- [ ] The full pipeline runs end-to-end with `geometry: true` on one small seed
      (smoke test in this milestone) and stays **off** by default everywhere else.
- [ ] The 90°-box assertion passes — the fidelity-spectrum claim is now proven code.
- [ ] All M02–M06 suites untouched and green; finish suites < ~25 s.
- [ ] Full root suite + typecheck green. STOP — hand off. Do not commit.

## Pitfalls

- Quantize hermite normals **before** the QEF accumulation — quantizing after vertex placement
  produces decimation-looking artifacts, not facets (this ordering is the whole trick).
- Rounding to 1e-3 happens in **cell-local** coordinates, before hashing *and* before emission —
  it is an output contract, not a test convenience.
- Vertex maps must be insertion-ordered (`Map`), keyed by cell coord strings — never object keys
  with numeric coercion.
