# 08 · Volumetric composition (L3 — organic shape)

> Each Space's abstract budget + envelope + sockets becomes an actual organic volume — as a
> **signed distance field**, not a mesh. SDFs keep every intermediate decision cheap,
> resolution-independent, and trivially combinable: noise-warping, biome displacement, smooth
> seam-blending, and recipe carve-ins are all just function composition. Triangles don't exist
> until L4.

## The SDF convention

`field(p): number` — **negative = open/walkable space, positive = solid matter**. A hull is the
open volume of one Space; an Area's field is the smooth union of its hulls and connector tubes.
All primitives are exact or Lipschitz-bounded distance functions so sphere-marching (socket
resolution, visibility rays) is valid.

### The primitive toolkit (normative formulas)

```
sphere(p, r)            = |p| − r
ellipsoid(p, r⃗)         ≈ (|p ∘ 1/r⃗| − 1) · min(r⃗)          (bound, adequate)
box(p, b⃗)               = |max(q⃗, 0)| + min(max(qx,qy,qz), 0)   where q⃗ = |p⃗| − b⃗
roundBox(p, b⃗, ρ)       = box(p, b⃗) − ρ
capsule(p, a⃗, b⃗, r)     = |p − a − clamp01(((p−a)·(b−a))/|b−a|²)(b−a)| − r
plane(p, n̂, h)          = p·n̂ + h
heightfield(p, h)       = p.z − h(p.x, p.y)                    (see Outdoor, below)

union(d₁, d₂)           = min(d₁, d₂)
intersect(d₁, d₂)       = max(d₁, d₂)
subtract(d₁, d₂)        = max(d₁, −d₂)
smoothUnion(d₁, d₂, k)  = min(d₁, d₂) − h²·k/4,  h = max(k − |d₁ − d₂|, 0)/k
displace(d, p)          = d(p) + amplitude · fbm3(p · frequency)   (deterministic noise, 02)

sdfNormal(f, p)         = normalize(∇f by central differences, ε = half a voxel)
```

Because hulls are *open* volumes, a Space's hull SDF is authored as the **negated** solid (e.g. an
open hall = `−roundBox(...)` carved from infinite rock); composition below keeps the sign
convention consistent: `areaField = smoothUnionAll(hullFields, connectorFields)` where every
component is negative inside its open volume.

## Hull archetypes — the hybrid registry

```ts
interface HullArchetypeDef {
  id: string;
  sdf: (p: Vec3, params: HullParams, rng: Rng) => number;  // pure, deterministic, seeded params only
  sizeRange: { min: Vec3; max: Vec3 };
  noise?: { amplitude: number; frequency: number };        // displacement defaults (biome may scale)
  kinds: ("room" | "outdoor")[];
  roles?: NodeRole[];                                       // e.g. a shaft suits segments, not vaults
  biomes?: string[];
  landmark?: boolean;
  weight: number;
}
```

Selection: filter by the Space's kind/role/biome/landmark flag and by `sizeRange` vs. the Space's
budget, then a seeded weighted pick. **Hybrid by design**: shipped procedural archetypes (`hall`,
`rotunda`, `cavern`, `shaft`, `gallery`, `bowl`) cover general Spaces; **host-authored landmark
archetypes** — hand-designed SDF compositions, or SDFs sampled from an authored mesh/voxel prefab —
provide the memorable set-pieces procedural variety can't (procedural alone reads as "big room,"
not "landmark"). Both enter through the same registry entry; nothing downstream knows the
difference.

The hull is sized to ~90% of the envelope (breathing room for smooth unions), displaced by
`noise` scaled by the biome, and then **recipe carves** ([06](./06-puzzles-locks-and-recipes.md))
are applied: named SDF edits (a hazard trench = subtract a shallow capsule from the floor; a high
ledge = union a shelf volume at `traversal.zUp`-derived height). Recipe carve-ins are part of hull
composition — not post-hoc mesh edits — so they inherit fidelity, collision, and determinism free.

## Outdoor Spaces

An outdoor Space is **sky-exposed**: its "walls" are terrain, not architecture.

```
ground   = heightfield(p, h)   where h(x,y) = base + fbm3(x·f, y·f, seedZ) · amp  (bowl-shaped:
           h rises toward the envelope rim so the space is naturally enclosed by cliffs)
openair  = ground ∪ (nothing above)  — no ceiling term at all inside the envelope's top face
rim      = the envelope's lateral faces get a cliff term (steep smoothstep of h toward max)
water?   = biome water level w: cells with z < w flagged for water dressing (09); `swim`
           traversal becomes legal below w
```

The composed field simply omits any ceiling: dual contouring then produces ground + cliff surfaces
and leaves the top open. The finish pass marks sky-exposed columns in Area meta (the host's
sky/lighting realizer reads this). Indoor↔outdoor transitions use `open-seam` connectors
([07](./07-spatial-skeleton.md)) — a widening cave-mouth tube whose far aperture blends into the
terrain via `smoothUnion` with a larger `k`.

Vistas: an outdoor Space with a landmark in its Area gets a `vista` content anchor placed on a
rim-adjacent surface with line-of-sight to the landmark (same LOS machinery as below) — the "see
it before you reach it" beat, exported as an anchor the host can dress (overlook, railing, glow).

## Connectors

A connector's hull is a swept tube along a Catmull–Rom spline:

```
control points: from-socket pos → 1–3 seeded intermediate points (jittered around the straight
                line, biased by kind: curved = lateral jitter, ramp = monotone z-lerp, shaft =
                vertical with a helical hint) → to-socket pos
tube(p) = capsuleSweep(p, spline, radius(t))    — radius from the narrower socket, ±seeded taper
kind-specific: ramp clamps slope ≤ its traversal's max; shaft is near-vertical with climbable
                radius; crawl uses the crawl aperture radius; open-seam flares its outdoor end
```

Sampled adaptively (arc-length steps ≤ half a voxel) into a polyline capsule-chain SDF — exact
enough for meshing, cheap to evaluate. The connector's `gate`/`oneWay`/`traversal` ride through to
its descriptor; its hull joins the area field via `smoothUnion` (seam constant `k` per biome).

## Socket resolution (provisional → resolved)

The precise algorithm for the L2→L3 hand-off ([01](./01-architecture.md), step 12):

1. For each provisional socket, sphere-march the **owning Space's hull field** from just outside
   the envelope along `−dir` until the surface (`|d| < ε`); that hit is the resolved `pos`.
2. Build the orientation basis: `forward = −sdfNormal(hull, pos)` (into the open volume), `up` =
   world-up projected off forward (fallback: any perpendicular for near-vertical apertures),
   `right = up × forward`. **Snap the basis to the fidelity grid** ([09](./09-naturalization-and-kit.md))
   so apertures agree with the faceting.
3. Carve the aperture: subtract a capsule (radius = socket width, axis along forward) from the
   area field at `pos` so the opening is guaranteed passable even where two hulls barely touch.
4. Record the resolved socket (pos, basis, radius, kind, traversal, signature, gate, oneWay) into
   the Space's descriptor. Provisional transforms never leave L3.

Unused degree slots (a Space rolled degree 4, only 3 connections materialized) are **walled off
diegetically**: the archetype may still carve a shallow alcove at the unused slot (dressing anchor
`rubble`/`debris` bound there) — reading as "there was something here," never as a missing door.

## Content anchors — scatter, don't hand-place

After the area field is composed, every gameplay-relevant placement binds to a **content anchor**
on real surface. One system serves items, interactables, lights, props, and dressing — anchors are
the same primitive as sockets minus connectivity.

### Surface classification

Any point on the field has an outward normal; classify it:

```ts
type SurfaceKind = "floor" | "wall" | "ceiling" | "slope" | "overhang";
// normal.z >  0.6 → floor      normal.z < −0.6 → ceiling
// |normal.z| ≤ 0.25 → wall     0.25 < normal.z ≤ 0.6 → slope    −0.6 ≤ normal.z < −0.25 → overhang
```

Representation-agnostic (needs only a normal), shared verbatim by the finish pass's piece
classification ([09](./09-naturalization-and-kit.md)) so anchors and geometry always agree.

### Poisson-disk scatter over the field surface

```ts
interface ContentAnchorKind {
  id: string;                        // "gadget-pickup" | "interactable" | "light" | "prop" |
                                     // "dressing" | "landmark-feature" | "vista" | "refill-site" | host-defined
  allowedSurfaces: SurfaceKind[];
  minSeparation: number;             // this kind's personal-space radius (world units)
  clearanceFromStructural: number;   // buffer kept around every socket/doorway
  targetDensity: number;             // anchors per unit surface area — or absolute count for sparse kinds
}
```

Algorithm (per Space, per kind, from `${spaceRoot}:anchors:${kindId}`):

1. **Candidates**: iterate the Space's voxel-scale surface cells (cells where the field changes
   sign — cheap; the finish pass reuses the same identification), in fixed grid order; jitter a
   point within each candidate cell (seeded) and project it onto the surface along the field
   gradient.
2. **Filter**: surface kind ∈ `allowedSurfaces`; field-verified clearance (the anchor's bounding
   radius must not poke back through the surface — one `field(p)` evaluation).
3. **Reject** any candidate closer than the stricter `minSeparation` to *any* accepted anchor of
   *any* kind, or within any socket's `clearanceFromStructural`.
4. **Accept** until `targetDensity`/count or the attempt budget is exhausted.

Required anchors (the Space's content manifest: gadget Locations, recipe-required interactables,
refill sites for `regenHint: "site"` resources) run **first** with a raised attempt budget; if a
required anchor cannot place, that's a `GenError` naming the Space and the kind — never a silent
drop ([12](./12-orchestration-and-host-integration.md)). Decorative kinds fill afterwards with
whatever room remains. An anchor's `kindId`/tags are the same shared vocabulary as everything else
— "what occupies this anchor" is signature matching, not a fourth system.

## Landmarks — memorability, deliberately

- Each Reach places 1–2 landmark Spaces ([07](./07-spatial-skeleton.md)); their hulls come from
  landmark archetypes (typically host-authored).
- **Visibility bias**: candidate landmark positions (seeded jitters of the layout position) are
  scored by line-of-sight — sphere-march the composed field from the candidate's crown toward
  each other Space's center; score = count of unobstructed rays. Pick the argmax (ties by id
  order). This is the concrete mechanism behind "orient yourself by the tower you can see from
  three rooms."
- A landmark hosts a `landmark-feature` anchor and is a preferred target for `gadget-pickup`
  anchors — memorability tied to progression, not set-dressing.

## Biomes — content packs, not palettes

```ts
interface BiomePack {
  id: string;
  palette: PaletteRamp;                       // color data for the host realizer
  materials: Record<SurfaceKind, string>;     // materialHint per surface kind (09)
  noise: { amplitude: number; frequency: number };   // hull displacement scaling
  hazards: string[];                          // hazard-set tags feeding lock pattern #11 (06)
  dressing: DressingSet;                      // allowed dressing kinds + weights (09)
  waterLevel?: number;
  outdoorAffinity: number;                    // scales L2's outdoorChance
  seamK: number;                              // smooth-union constant at this biome's joins
  subBiomes?: { id: string; weight: number }[];
}

interface BiomePlanConfig {
  biomesPerReach: { min: number; max: number };
  areaSwapChance: number;    // hard cut at Area granularity: the NEXT Area switches biome outright
  gradientBlend: boolean;    // soft within-Area sub-biome blending
}
```

Two distinct transition tools, both shipped:

- **Sub-biome gradient** (within an Area): a weight field over the Area's extent blends two packs'
  noise/dressing/palette parameters — a transition the player *discovers*, not a seam they notice.
- **Area swap** (between Areas): at `areaSwapChance` the next Area cuts to a different pack from
  the Reach's palette — the classic hard biome change, with the connecting connector's dressing
  interpolating the two.

Every Area additionally samples a small seeded deviation of its pack's parameters — two Areas in
one biome never look identical.

## What L3 hands to L4

Per Area: the composed `field`, its voxelization extents + resolution, every Space's resolved
sockets and accepted anchors, biome/blend data, sky-exposure info, and the poly/piece budgets. L4
converts, classifies, and exports — it makes no decisions ([09](./09-naturalization-and-kit.md)).
