# M06 · Volumetric composition & content anchors (L3)

**Goal**: the `volume/` and `anchors/` modules — SDF hulls per Space (including outdoor
heightfields and authored landmark archetypes), spline connectors, the composed per-Area field,
**socket resolution** (provisional → on-surface, fidelity-snapped), Poisson-disk content-anchor
scatter with manifest binding, landmark line-of-sight placement, and biome blending.

**Required reading**: [redesign 08 (all)](../redesign/08-volumetric-composition.md);
[redesign 01 §Master order steps 11–14](../redesign/01-architecture.md).

**Prerequisites**: M05 green.

**Porting references**: legacy `core/src/volume/{sdf,hulls,spline,field}.ts` (primitives/ops and
the Catmull–Rom + capsule sweep are proven — port, then extend per spec). The legacy
noUncheckedIndexedAccess fix in `spline.ts` (the literal-index `cr()` helper for Catmull–Rom
components) is the required pattern — keep it.

## Phase 6.1 — `volume/sdf.ts`

Port + verify every primitive/op against the formula block in redesign 08 (sphere, ellipsoid,
box, roundBox, capsule, plane, heightfield; union/intersect/subtract/smoothUnion/displace;
`sdfNormal` central differences with ε = half a voxel). `Sdf = (p: Vec3) => number`. Sign
convention: **negative = open**. Unit tests: exact distances at hand-picked points for each
primitive; smoothUnion ≤ min everywhere on a sample grid; displace determinism.

## Phase 6.2 — `volume/hulls.ts` + `volume/outdoor.ts`

`HullArchetypeDef` exactly per redesign 08. Shipped procedural archetypes: `hall` (roundBox),
`rotunda` (cylinder-ish via capsule/ellipsoid union), `cavern` (displaced ellipsoid union),
`shaft` (vertical capsule), `gallery` (elongated roundBox + column subtractions), `bowl`
(outdoor). `selectArchetype(registry, spec, rng)`: filter by kind/role/biome/landmark/sizeRange
vs. budget → seeded weighted pick. `buildHull(archetype, spec, biome, rng): Sdf` — size to 90% of
the envelope, apply biome-scaled noise displacement, then apply the Space's **recipe carves**
(named carve library: `hazard-trench` = subtract shallow floor capsule; `ledge-shelf` = union a
shelf at a bucket-derived height; add the carves the shipped recipes reference).

`outdoor.ts`: `buildOutdoorHull(spec, biome, rng): Sdf` per redesign 08 — fbm heightfield rising
toward the rim (bowl), no ceiling term inside the envelope top, optional water level flag-through.

## Phase 6.3 — `volume/spline.ts` + `volume/field.ts`

Spline connectors per redesign 08: control points (socket → 1–3 seeded intermediates by kind →
socket), Catmull–Rom sampling at arc-length ≤ half a voxel into a polyline capsule-chain SDF;
kind constraints (ramp slope clamp, shaft near-vertical, crawl radius, open-seam flare).

`field.ts`: `composeAreaField(hulls, connectors, biome): AreaField` — smooth-union all (per-biome
`seamK`), plus `carveAperture(field, socket)` (subtract a capsule along the socket forward axis)
applied for every resolved socket. `AreaField` carries the callable field + voxelization extents
(union of envelopes + margin) + res (from the fidelity profile, `maxDim`-clamped — record when
clamped).

## Phase 6.4 — Socket resolution: `volume/socket-resolve.ts`

The 4-step algorithm from redesign 08 verbatim: sphere-march inward along `−dir` from outside the
envelope (step = |d|, cap 64 steps, accept |d| < 1e-3); basis = `forward = −sdfNormal`, up =
world-up projected (fallback perpendicular when |forward·up| > 0.98), right = up × forward;
**snap the basis via the fidelity quantizer** — M07 owns `quantizeNormal`, so to avoid a forward
dependency, put `quantizeNormal` in `math/` now (move it here in this milestone; M07 re-exports)
— it only needs `datan2`/`dsin`/`dcos`. Carve the aperture. Unused degree slots: carve a shallow
alcove + bind a `rubble` dressing hint on the spec.

## Phase 6.5 — `anchors/`

`surface-classify.ts` — the `SurfaceKind` cones exactly per redesign 08 (thresholds 0.6/0.25).
`anchor-kinds.ts` — `ContentAnchorKind` shape + shipped defaults for
`gadget-pickup | interactable | light | prop | dressing | landmark-feature | vista | refill-site`.
`scatter.ts` — the Poisson algorithm per redesign 08: iterate sign-change surface cells in fixed
grid order, seeded jitter, gradient projection, filter by allowed surface + field clearance,
reject by cross-kind `minSeparation` + `clearanceFromStructural` + hull poke-through; **required
manifest anchors first** with raised attempt budget (×8) — failure ⇒ `GenError` naming Space +
kind. Fork: `${spaceRoot}:anchors:${kindId}`.
`landmarks.ts` — LOS scoring per redesign 08: sphere-march candidate-crown → other-Space centers
through the composed field; argmax with id-order tiebreak; `vista` anchors on rim surfaces with
LOS to the landmark.

## Phase 6.6 — Wire into the pipeline

`ReachResult.areas[i]` gains `field: AreaField`, `resolvedSockets`, `anchors: ContentAnchor[]`
(the output shape from redesign 10, bindings included), `biomeBlend`. This runs whenever the
skeleton runs (fields are cheap to *build* — only meshing is heavy); the `geometry` flag still
only gates M07.

## Phase 6.7 — Tests (`volume/volume.test.ts`, `anchors/anchors.test.ts`)

- SDF unit tests (Phase 6.1) + hull sizing: hull open-volume stays inside the envelope (sample
  grid: no negative field value outside envelope + margin).
- Outdoor: above-terrain samples open all the way to the envelope top; rim samples solid.
- Socket resolution: for 50 seeds × a two-room fixture — every resolved socket has |field| <
  1e-3, orthonormal fidelity-snapped basis, and its aperture is passable (a straight-line walk of
  field samples along forward through the aperture stays negative).
- Connector: field along the sampled spline polyline is negative end-to-end (the corridor is
  actually open); ramp slope ≤ clamp.
- Anchors: the three overlap invariants from redesign 15 (cross-kind separation, structural
  clearance, no poke-through) over 50 seeds; required-anchor failure fixture (an absurd
  `minSeparation`) throws `GenError` naming the kind.
- Landmark LOS: in a fixture with one open bowl + one buried room, the landmark lands in the
  bowl (deterministically).
- Determinism: field sampled on a fixed lattice twice → identical; anchors twice → identical.

Keep every fixture tiny (2–4 Spaces, dims ≤ 32³) — this is the milestone where test cost starts
to bite; the perf discipline from the README applies from here on.

## Definition of Done

- [ ] Socket resolution meets the redesign 15 invariants (on-surface, snapped, passable) in
      tests.
- [ ] Required manifest anchors always place or throw — never silently missing.
- [ ] `quantizeNormal` lives in `math/`, tested (azimuth/elevation snap at 5° and 90°).
- [ ] Full root suite + typecheck green; new suites < ~20 s total. STOP — hand off. Do not
      commit.

## Pitfalls

- Sphere-marching a *non-exact* SDF (smoothUnion results are bounds): step by `max(|d|, minStep)`
  with `minStep = res/4` to guarantee progress.
- The scatter's fixed grid order must iterate x→y→z exactly like the mesher will (M07) — share a
  single `forEachCell(extents, fn)` helper in `volume/field.ts` now.
- Never evaluate the field with host trig inside archetype SDFs — rotations use `yawBasis` from
  `math/`.
