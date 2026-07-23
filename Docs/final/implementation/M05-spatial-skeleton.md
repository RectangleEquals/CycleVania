# M05 · Spatial skeleton (L2)

**Goal**: the `skeleton/` module — each Region becomes an Area of `SpaceSpec`s with real origins,
budgets, degrees, provisional sockets, connector plans, and all abstract flags (**outdoor and
landmark are decided here**, testably, with geometry off). After this milestone,
`requestReach(...)` returns Areas full of positioned, wired, still-abstract Spaces.

**Required reading**: [redesign 07 (all)](../redesign/07-spatial-skeleton.md);
[redesign 01 §Master order steps 8–10](../redesign/01-architecture.md).

**Prerequisites**: M04 green.

**Porting reference**: legacy `core/src/layout/forcedirect.ts` (the force loop is proven — port
its body-object accumulation pattern; extend per spec with z-plan and pinning).

## Phase 5.1 — Budget → dials: `skeleton/area-dials.ts`

Pure function `deriveAreaDials(budgetSlice, buckets, biome, role, dials, rng): AreaDials`
implementing redesign 07's derivation table. Contractual specifics:

- `zSpread = zBase + K_Z * Math.sqrt(buckets["traversal.zUp"] ?? 0)` (diminishing returns —
  `Math.sqrt` is allowed math).
- `spaceCount` drawn in a role-dependent range scaled by the slice (capstone: fewer, larger).
- `outdoorChance = base * biome.outdoorAffinity`, zero when the biome vetoes.
- Every derived number recorded in the Area's meta (tooling reads *why*).

Budget cascade in `skeleton/budget.ts`: `splitReachBudget(finalCeiling, areaCount, roles, rng)`
(uneven seeded weights, capstone-heavy) and the per-Area pooling helper
`poolShare(remaining, spaceIndex, rng)` per redesign 04/07.

## Phase 5.2 — Space planning: `skeleton/space-plan.ts`

`planSpaces(region, dials, manifest, rng): SpaceSpec[]` — `SpaceSpec` and `SpaceBudget` exactly
per redesign 07. Steps: draw `spaceCount`; assign roles (entry Space; boss Space in capstone
Areas); roll `outdoor` (only `segment`/`hub` roles, larger budgets); flag `landmark` per the
Reach-level pick (the ReachComposer chooses 1–2 landmark Spaces across the Reach, biased to hubs
— implement as a pre-pass in `reach-skeleton.ts` below); reserve recipe envelopes: every
`PuzzleInstance` bound to this Region with a `spatialRecipe` claims a qualifying Space (create an
extra Space if none qualifies — before layout, recorded); attach the content manifest (Locations,
gadget pickups, refill sites) per Space.

Degrees: `rollDegrees(spec, degreeTable, rng)`; then **capacity validation**: L1 edges incident
to the Region vs. total structural capacity; insert `junction` Spaces (role `"junction"`, degree
3) until it fits, recorded in relaxations.

## Phase 5.3 — Layout: `skeleton/force-layout.ts` + `skeleton/z-plan.ts`

`zPlan(spaces, edges, maxZDepth, rng)`: seeded target-Z bands honoring drop/climb edge
directions (drop target strictly lower). `forceLayout(nodes, edges, opts, rng)` per redesign
07's pseudocode: fixed 220 iterations, springs (`restLength` = radii sum + connector length
draw), envelope-aware repulsion, `zSeparation` pull toward the plan, damping + step clamp,
pinning (entry Space; portal Spaces toward neighbor Areas), then deterministic residual-overlap
push (by node-id order) and 1e-3 quantization. Constants (`K_SPRING 0.06`, `K_REPEL 1.4`,
`DAMPING 0.85`, `MAX_STEP 0.9`, iterations 220) live in the dial surface.

The same function runs once at Reach scope (`reach-skeleton.ts`) to position Areas relative to
each other (Areas as nodes, cross-Area edges as springs) — portal pin directions come from that
pass.

## Phase 5.4 — Sockets & connectors: `skeleton/socket-wiring.ts`, `skeleton/connector-plan.ts`

Per redesign 07: for each planned connection, pick signature-compatible socket slots
(`SignatureConfig.compatible` score ≥ `1 − fuzziness`; default exact match), place
**provisional** sockets on envelope boundaries toward the partner (seeded cone jitter ≤ 20°),
assign traversal from Δz thresholds (|Δz| > 0.6·envelope height ⇒ `drop`/`climb`; biome may
substitute per its sets), carry the L1 edge's `gate`/`oneWay` **by reference**. Connector plans:
kind from the weighted distribution constrained by traversal; per-traversal `lengthBounds`; a
span > `max` inserts a waypoint junction (recorded). Indoor↔outdoor joins force `open-seam`.

`secretFraction`: mark that fraction of non-critical Locations' host Spaces `hidden`-biased
(socket prefers `crawl`/`revealable`-signature slots). Secrets must already be `bonus`/optional —
assert, don't re-class.

## Phase 5.5 — Wire into the pipeline

`ReachComposer` (in `world/`) now continues past step 7: `reach-skeleton.ts` lays out Areas,
then per Region runs dials → plan → layout → wiring, storing the results on `ReachResult` as
`areas: AreaSkeleton[]` (`{ regionId, dials, spaces: SpaceSpec[], connectors: ConnectorSpec[],
relaxations }`). Geometry flag is irrelevant here — the skeleton always builds (it's cheap).

## Phase 5.6 — Tests (`skeleton/skeleton.test.ts` — all with geometry off)

- **No overlaps**: 200 seeds × classic-ish synthetic registry — no two Space envelopes in an
  Area overlap; no two Area bounds in a Reach overlap.
- **Capacity**: every L1 edge has exactly one connector plan; dense-hub fixture (7 edges into a
  degree-3 Region) auto-inserts junctions and drops nothing.
- **Outdoor at L2**: over 200 seeds with `outdoorChance > 0`, outdoor Spaces appear, only on
  legal roles, and **this assertion runs without any geometry module imported** (the layer-
  contract regression test).
- **Landmarks**: 1–2 per Reach, biased to hubs (distributional).
- **Gate fidelity**: every socket/connector `gate` is **object-identical** (`===`) to its L1
  edge rule.
- **Z**: a drop edge's target Space origin is strictly lower; `zSpread` responds to a fat
  `traversal.zUp` bucket (two-config comparison) with the sqrt shape (4× bucket ⇒ ~2× spread ± tolerance).
- **Determinism**: full skeleton for one seed, twice → stable-JSON identical.
- **Length bounds**: an artificially stretched pair (pin two Spaces far apart) inserts a
  waypoint rather than a too-long connector.

## Definition of Done

- [ ] `requestReach` returns skeletons; M02–M04 suites untouched and green.
- [ ] All layout/wiring randomness drawn from `${reachRoot}:area:${regionId}` forks (grep the
      fork labels).
- [ ] All relaxations (junctions, waypoints, recipe-space creation) recorded, never silent —
      each also emitted as a `warn` diagnostic with the same stable code and a full `path`
      (`reachN/area:rX`), via a `Diag.child` threaded through the composers.
- [ ] Full root suite + typecheck green; skeleton soak < ~15 s. STOP — hand off. Do not commit.

## Pitfalls

- Force accumulation on typed arrays fights `noUncheckedIndexedAccess` — use per-body object
  fields (`b.fx += …`), as the legacy port does.
- Iterate node pairs in fixed (id-sorted) order for repulsion — `Map` iteration over insertion
  order is fine, but never iterate a `Set` built from unordered sources.
- The provisional socket cone jitter must come from the *wiring* fork, not the layout fork —
  keep the streams separate so tuning one never reshuffles the other.
