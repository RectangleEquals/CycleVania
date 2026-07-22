# 07 · Spatial skeleton (L2 — wiring an Area in space)

> The first layer that touches coordinates. Each Region becomes an **Area** of Spaces: the
> `AreaComposer` decides how many Spaces, of what kinds (including outdoor and landmark), with what
> budgets, *where* they sit (force-directed layout), and how they connect (degree rolls + socket
> pairing) — all while everything is still abstract envelopes and provisional transforms. No hull
> exists yet; nothing here can touch solvability.

## Inputs (from L1) and outputs (to L3)

**In**: one Region (+ its role), the Area's budget slice, the Reach's placement (which Locations,
gadgets, and Puzzle instances live here), reserved recipe envelopes
([06](./06-puzzles-locks-and-recipes.md)), the biome plan, and the aggregated capability buckets
([05](./05-capabilities-and-facets.md)).

**Out**: a list of `SpaceSpec`s (id, kind, role, budget, origin, bounding envelope, provisional
sockets, biome, flags like `outdoor`/`landmark`), a list of `ConnectorSpec`s (socket pair, kind,
traversal, gate carried from the L1 edge), and intra-Area portal specs for cross-Area edges.

## Deriving the Area's dials from its budget

The Area's `ComplexityBudget` slice is decomposed into concrete dials by pure functions of
`(budget, buckets, biome, role, rng)`:

| Derived dial | Driven by |
|---|---|
| `spaceCount {min,target,max}` | budget magnitude, role (capstone fewer+larger) |
| `zSpread` / `MaxZDepth` | budget + `traversal.zUp`/`traversal.zDown` bucket aggregates — **the world-shaping loop lands here** |
| `loopDensity` | template `loops.density` + `traversal.zDown` (drop-loops) + modifier nudges |
| `largeSpaceChance` / `outdoorChance` | budget + biome (`BiomePack.outdoorAffinity`) |
| `secretFraction` | registry default + modifier `reward.bonusLocations` |
| `hazardDensity` | hazard baseline curve ([04](./04-worlds-reaches-and-pacing.md)) × biome hazard set × modifier `hazard.densityMul` |
| connector kind weights | biome + budget (more vertical kinds as `zSpread` rises) |

Verticality gets **diminishing returns by default**: `zSpread` grows sub-linearly with the z
buckets (`zSpread = base + K * sqrt(zUpAggregate)`), keeping worlds lateral-dominant unless the
host retunes — vertical is an accent, not a takeover, matching how the reference games use it.

## Space kinds — decided here, realized later

```ts
type SpaceKind = "room" | "outdoor" | "connector";

interface SpaceSpec {
  id: string;
  kind: SpaceKind;
  role: NodeRole | "junction";          // junction: inserted by capacity validation (below)
  budget: SpaceBudget;
  origin: Vec3;                          // set by layout
  envelope: WorldBox;                    // bounding volume, set by layout + budget
  sockets: ProvisionalSocket[];
  biome: string;
  sub?: { biome: string; blend: number };// sub-biome gradient endpoint (08)
  outdoor: boolean;
  landmark: boolean;
  reservedRecipes: string[];             // SpatialRecipeDef ids bound to this Space
}

interface SpaceBudget {
  volumeCells: number;                   // voxel-scale volume ceiling
  polyAllowance: number;                 // finish-pass budget share (09)
  degree: { min: number; max: number };  // structural-socket count range
}
```

**All abstract Space facts are decided at L2** — normatively including `outdoor` and `landmark`.
(An earlier implementation let the geometry pass roll outdoor-ness; that inverted the layer
contract and made abstract facts untestable without running heavy geometry. Forbidden here:
[01](./01-architecture.md), master-sequence rules.)

- **Outdoor**: rolled per Space at `outdoorChance`, only for `segment`/`hub`-roled, larger-budget
  Spaces (a vault is enclosed by design); biome can veto or force. An outdoor Space's envelope is
  sky-open (its top face carries no ceiling constraint into L3).
- **Landmark**: per Reach, 1–2 Spaces (dial) are flagged landmark — biased toward `hub` roles and
  the high end of the degree range; they draw from landmark hull archetypes and get the
  visibility-biased placement pass in L3 ([08](./08-volumetric-composition.md)).
- **Boss**: the capstone Region's Area always contains a boss Space — `room` kind ⇒ BossChamber,
  `outdoor` kind ⇒ BossArena — with the `boss-arena` recipe reserved.

## Connectivity degrees

Every Space's structural-socket count is rolled within its kind/role's range — this decides
pass-through vs. many-armed hub, and it's a host-tunable table, not a constant:

| Space kind/role | default degree | why |
|---|---|---|
| connector | exactly 2 | it exists to join exactly two things |
| ordinary room | 2–3 | the metroidvania default |
| hub | 3–6 | deliberately a decision point |
| vault / dead end | 1 | a dead end by design |
| landmark | up to 6–8 (host ceiling) | the memorable "lay of the land" chamber |

Bounding the degree matters structurally, not just aesthetically: every extra socket is another
compatibility constraint to solve, and an unbounded-degree Region makes the mission graph illegible
to both tooling and humans. Allow it, and bound it — per kind, host-declared.

**Capacity validation**: the L1 graph's edges incident to this Region must fit within the rolled
degrees. If the Region's edge count exceeds what its Spaces offer, the composer **auto-inserts
`junction` Spaces** (small, role `"junction"`, degree 3) until capacity fits — deterministic,
recorded in Area meta. A dense hub therefore never mis-gates a door by silently dropping an edge.

## Force-directed layout

Positions come from a bounded, seeded force simulation over the Area's Space graph (the intra-Area
connectivity graph: one node per non-connector Space, one edge per planned connection). Why not a
tree fan-out: fan-outs produce corridors-off-a-spine geometry that reads instantly as generated;
force layout finds where Spaces "want" to sit given their connections, producing organic adjacency
and natural loop geometry.

```
inputs: nodes (with envelope radii, pin flags), edges (with rest lengths), opts, rng fork
state per node: pos (seeded jittered start), force accumulator

iterate FIXED n times (default 220):
  for each edge (a,b):                     # springs
    f = K_SPRING * (dist(a,b) − restLength(a,b));  apply ±f along the a→b axis
  for each pair (a,b):                     # repulsion (envelope-aware)
    if dist < (rA + rB + margin): apply K_REPEL / dist² pushing apart
  for each node:                           # verticality bias
    apply zSeparation * sign(targetZ(node) − pos.z)   # targetZ from the Area's z plan (below)
  for each node:                           # integrate
    pos += clamp(force * DAMPING, MAX_STEP); pinned nodes don't move
finally: shift all positions so the entry Space sits at the Area origin; quantize to 1e-3
```

- `restLength(a,b)` = sum of the two envelope radii + the planned connector's length draw (below).
- **Pinning**: the entry Space is pinned; Spaces hosting a cross-Area portal are pinned toward the
  neighboring Area's direction (from the Reach-level area layout — the same algorithm runs once at
  Reach scope to position Areas relative to each other).
- **Z plan**: before layout, Spaces are assigned target Z bands (seeded, within `MaxZDepth`,
  respecting one-way-drop edges: a drop edge's target has lower Z than its source; a climb-gated
  edge the reverse). `zSeparation` pulls layout toward the plan without hard-constraining it.
- Fixed iteration count + fixed traversal order + seeded jitter ⇒ fully deterministic
  ([02](./02-determinism.md)).
- **Overlap resolution**: after iteration, any residual envelope overlap is resolved by the
  minimal axis push (deterministic order: by node id); layout never fails — it only spreads.

## Sockets at L2 — provisional transforms

```ts
interface ProvisionalSocket {
  id: string;
  spaceId: string;
  pos: Vec3;                 // ON the envelope boundary, facing the partner
  dir: Vec3;                 // outward, toward the partner Space
  kind: "structural";
  traversal: Traversal;      // walk | climb | crawl | drop | swim | vertical
  signature: string;         // compatibility tag (below)
  gate?: Rule;               // carried VERBATIM from the L1 edge — never re-derived
  oneWay?: boolean;
  partner: { spaceId: string; socketId: string };
}
```

Pairing algorithm, per planned connection: roll each endpoint Space's available socket slot
(respecting its remaining degree), choose a `signature` compatible pair (below), place each
provisional socket on its envelope boundary at the intersection with the line toward the partner
(jittered within a seeded cone so connections aren't laser-straight), assign `traversal` from the
Z relationship (Δz below/above thresholds ⇒ `drop`/`climb`/`vertical`; else `walk`, biome may
substitute `swim`/`crawl` per its sets).

### Signatures — a lightweight WFC

Rather than hand-authoring an adjacency table for every hull × hull pairing (combinatorial, and
the biggest source of visible seams), every socket carries a short compatibility `signature`
(e.g. `"wide-arch|floor-level"`). Pairing only considers signature-compatible pairs — a minimal
Wave-Function-Collapse-style local constraint guaranteeing a hall never plugs into a mismatched
opening. The same vocabulary strings are used by TagFacets and recipes
([05](./05-capabilities-and-facets.md), [06](./06-puzzles-locks-and-recipes.md)) — one namespace,
validated at registry load.

```ts
interface SignatureConfig {
  compatible: (a: string, b: string) => number;  // 0..1 score; default: exact-match = 1 else 0
  fuzziness: number;                             // accept pairs with score ≥ 1 − fuzziness (default 0 = exact)
}
```

`fuzziness` is the dial trading "always geometrically safe" for "more visually varied
connections" as a spectrum, not all-or-nothing.

## Connector planning

```ts
interface ConnectorSpec {
  id: string;
  from: SocketRef; to: SocketRef;
  kind: "straight" | "curved" | "ramp" | "shaft" | "crawl" | "open-seam";
  traversal: Traversal;
  lengthBounds: { min: number; max: number };   // per-traversal host config
  gate?: Rule; oneWay?: boolean;
}
```

- Kind is drawn from the biome/budget-weighted distribution; `traversal` constrains it (a `drop`
  is a shaft; a `crawl` a crawl-tube; an outdoor↔indoor join prefers `open-seam` — the cave-mouth /
  collapsed-wall transition, so outdoor Spaces are entered *diegetically*, not through a door).
- **Length bounds are per-traversal** (a crawl shouldn't span like a walk); if layout left two
  sockets further apart than `max`, the connector plan inserts a waypoint (a mini `junction`) —
  again deterministic and recorded.
- The L1 edge's `gate`/`oneWay` ride on the connector (and its sockets) verbatim. `requiredCaps`
  surfaces via `missingCaps` on the full Rule — never collapsed to one "primary" capability.

## Secrets

`secretFraction` (0..1, default per preset) marks that fraction of each Area's non-critical
Locations as secrets: their hosting Space gets a `hidden` bias — the socket leading to it prefers
`crawl`/`revealable` signatures, and the finish pass tags the aperture's pieces `revealable`
([09](./09-naturalization-and-kit.md)) when a perception-tag capability is in the World's catalog.
Secrets are always `bonus`/optional-class content — the solvability proof never depends on finding
one.

## What L2 hands to each SpaceComposer

Bundled per Space: its `SpaceSpec` (budget, envelope, kind, flags, biome + sub-biome blend), its
provisional sockets, its bound recipes, and its content manifest (which placed Locations, gadget
pickups, and puzzle anchors must physically exist inside it). L3 realizes exactly this manifest —
it may not add or drop gameplay-relevant content, only decorate.
