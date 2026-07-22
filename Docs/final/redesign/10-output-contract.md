# 10 · Output contract (descriptors, serialization, the realizer guide)

> Everything CycleVania produces, as exact shapes. The contract is: **plain, serializable,
> self-describing data** — `number[]` buffers, string ids, no engine types, no functions, no
> `undefined`-vs-missing ambiguity (optional fields are absent, never `null`). A host that stores
> a descriptor and reloads it a year later has everything, including the provenance needed to
> regenerate it from scratch.

## The descriptor tree

```
WorldDescriptor
├─ meta: WorldMeta                        (seed, fingerprint, versions, request log)
├─ reachPortals: ReachPortal[]            (cross-Reach navigation, 04)
└─ reaches: ReachDescriptor[]             (realized only — sparse by reachIndex)
   ├─ meta: ReachMeta                     (requestIdentity, FinalCeiling, buckets, spheres…)
   ├─ graph: MissionGraphData             (regions, edges+rules, flags, locations)
   ├─ placement: PlacementData            (locationId → itemId, sphere indices)
   ├─ puzzles: PuzzleInstanceData[]       (defId, boundRegion, boundSpace, conditionRef, outcome)
   └─ areas: AreaDescriptor[]
      ├─ areaId, regionId, role, biome, bounds, seedPath
      ├─ spaces: SpaceDescriptor[]
      ├─ connectors: ConnectorDescriptor[]
      ├─ portals: PortalSpec[]            (cross-AREA links within the Reach)
      ├─ anchors: ContentAnchor[]         (all accepted L3 anchors, incl. required/manifest bindings)
      ├─ kit?: GeneratedKit               (present iff geometry ran)
      ├─ instances?: PieceInstance[]
      ├─ occupancy?: OccupancyData
      └─ dressing?: DressingAnchor[]
```

### Shapes

```ts
interface WorldMeta {
  worldSeed: string;
  generationVersion: string;            // CycleVania algorithm version (02)
  registryFingerprint: string;          // content hash of the validated registry (02)
  lengthPolicy: { min: number; max?: number };
  drawnLength?: number;                 // L, when bounded
  requestLog: ReachRequestRecord[];     // the reproducibility unit's second half — canonical order
}

interface ReachMeta {
  reachIndex: number;
  requestIdentity: string;              // canonical hash of the full ReachRequest
  chosenModifiers: string[];
  finalCeiling: number;
  areaCount: number;
  buckets: Record<string, number>;      // aggregated capability magnitudes at this Reach (05)
  spheres: string[][];                  // locationIds per sphere index
  relaxations: string[];                // any documented cap relaxations / junction insertions (03, 07)
  startHeld: HeldData;                  // the concrete carried state this Reach was proven against
}

interface SpaceDescriptor {
  id: string;
  kind: "room" | "outdoor";             // connectors get their own ConnectorDescriptor list
  role: string;                         // NodeRole | "junction"
  regionId: string;
  origin: [number, number, number];
  bounds: WorldBoxData;
  outdoor: boolean;
  landmark: boolean;
  biome: string;
  subBiome?: { id: string; blend: number };
  hullArchetype: string;
  sockets: SocketData[];                // RESOLVED sockets only
  recipeIds: string[];                  // SpatialRecipeDef ids realized here
  skyOpen?: boolean;
}

interface SocketData {
  id: string;
  pos: [number, number, number];
  basis: { forward: Vec3Data; up: Vec3Data; right: Vec3Data };  // fidelity-snapped (08)
  radius: number;
  kind: "structural" | "content";
  traversal: Traversal;
  signature: string;
  gate?: RuleData;                      // the FULL rule — UIs derive requiredCaps via missingCaps
  oneWay?: boolean;
  partner?: { spaceId: string; socketId: string };
}

interface ConnectorDescriptor {
  id: string;
  from: SocketRef; to: SocketRef;
  kind: "straight" | "curved" | "ramp" | "shaft" | "crawl" | "open-seam";
  traversal: Traversal;
  spline: [number, number, number][];   // the control polyline (host may re-dress the tube)
  gate?: RuleData; oneWay?: boolean;
}

interface ContentAnchor {
  id: string;
  spaceId: string;
  kindId: string;                       // ContentAnchorKind id
  pos: [number, number, number];
  up: [number, number, number];
  surface: SurfaceKind;
  binding?:                             // present when the anchor realizes gameplay content
    | { type: "location"; locationId: string; itemId?: string; sphere: number }
    | { type: "puzzle"; puzzleInstanceId: string }
    | { type: "refill"; poolId: string }
    | { type: "landmark" } | { type: "vista"; landmarkSpaceId: string };
  tags: string[];
}

interface OccupancyData { origin: [number, number, number]; res: number;
                          dims: [number, number, number]; solid: number[]; }
```

`GeneratedKit`/`GeneratedPiece`/`PieceInstance`/`DressingAnchor` are as defined in
[09](./09-naturalization-and-kit.md). `MissionGraphData`/`RuleData` are the direct serializations
of [03](./03-mission-graph.md)'s shapes (rules as tagged unions — trivially JSON).

## Serialization rules

- **JSON-stable**: every descriptor round-trips `JSON.parse(JSON.stringify(x))` bit-identically.
  Buffers are `number[]` with the 1e-3 rounding contract ([09](./09-naturalization-and-kit.md));
  hosts convert to `Float32Array`/typed arrays at load (`toTypedKit(kit)` helper ships in core).
- **Canonical key order** for hashing/diffing: serializers emit keys in schema order, arrays in
  the deterministic generation order — two equal worlds produce byte-equal JSON.
- **Sparse by realization**: `reaches` contains only realized Reaches; nothing about unrealized
  slots is ever present (there is nothing to say about them beyond `previewReachEnvelope`).
- **Additivity**: geometry fields (`kit`/`instances`/`occupancy`/`dressing`) are optional and
  layered on top — a `geometry: false` run yields a complete, simulatable, later-upgradeable
  descriptor (re-running the finish pass on the same seed adds byte-identical geometry).

## The realizer guide (host-side, step by step)

A realizer is typically < 200 lines. The reference realizer ships in the Inspector; a host's
differs only in engine API names.

1. **Kit → engine meshes**: for each `GeneratedPiece`, build one engine geometry from
   positions/normals/indices (flat-shaded for faceted profiles). Pick a material by
   `meta.materialHint` + `meta.biome` (the `BiomePack.palette` you supplied is echoed in the
   registry — your material table keys off your own hints).
2. **Instances → placement**: for each `PieceInstance`, place the piece at
   `areaOrigin + coord * kit.cellSize`, rotated by `yaw`. Use hardware instancing per pieceId —
   the kit is designed for it.
3. **Collision**: either feed `occupancy` to `collideSphere` (shipped, deterministic, good enough
   for character controllers), or build engine colliders from pieces with
   `meta.collider === "solid"`. Pieces tagged `revealable` start non-colliding; your runtime
   toggles them on the perception event.
4. **Sockets & portals**: wire your door/transition prefabs at structural sockets that carry a
   `gate` (evaluate/display via `evalRule`/`missingCaps` against your live `Held`); `open-seam`
   connectors and ungated sockets need no prefab at all. Cross-Area `PortalSpec`s and
   `ReachPortal`s drive your streaming/transition system.
5. **Anchors → content**: spawn your actual pickups/interactables/lights at `ContentAnchor`s by
   `kindId` + `binding` (a `location` binding tells you which item of yours to spawn and its
   sphere for hint UIs; a `puzzle` binding which puzzle instance; `refill` which resource pool).
   `dressing` anchors are optional garnish — skip them entirely and the world still plays.
6. **Sky & water**: `skyOpen` Spaces and water levels come through Space/dressing data; your sky,
   fog, and water rendering are entirely yours.

## Streaming, persistence & multiplayer

- **The Area is the streaming unit** — self-contained geometry + collision + anchors, generated
  from its own RNG fork, loadable/unloadable independently. A Reach streams Area-by-Area; the
  first Area can be realized while later ones still generate (orchestration,
  [12](./12-orchestration-and-host-integration.md)).
- **Two multiplayer models, both supported**: (a) *ship descriptors* — the server generates and
  sends the serialized descriptor (compact: kit dedup keeps it small; occupancy compresses well);
  (b) *regenerate from identity* — the server sends `(worldSeed, registryFingerprint,
  generationVersion, requestLog)` and clients regenerate byte-identically. Model (b) requires
  clients to hold the same registry data at the same fingerprint — which the fingerprint makes
  checkable up front.
- **Persistence** stores either the descriptor or the identity — they are equivalent by the
  determinism contract, and `WorldMeta` always carries the identity either way, so persisted
  worlds remain regenerable and diffable forever.
