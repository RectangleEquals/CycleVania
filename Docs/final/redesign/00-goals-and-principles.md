# 00 · Goals & principles

## The one-sentence pitch

CycleVania turns **a seed + host-supplied game data** into a **provably-solvable, deterministic,
engine-agnostic 3D metroidvania world — including its actual level geometry** — by keeping five
separate concerns in five separate layers that never leak into each other: **data** (what content
exists), **logic** (is it solvable?), **skeleton** (how is it wired in space?), **volume** (what
does it organically look like?), and **finish** (what do you actually render and collide with?).

A host game supplies content as data (capabilities, puzzles, biomes, hull shapes, dials) and gets
back a complete, serializable world description: a guaranteed-solvable progression graph, an
organic 3D layout, real triangle geometry split into a deduplicated modular kit, a collision grid,
and content anchors — everything except textures, gameplay code, and fiction.

## The six product goals

These are the goals every design decision in this suite serves. When in doubt during
implementation, decide in favor of these, in this order:

1. **Any host, any scale.** Comprehensive enough that any aspiring 3D metroidvania project — from a
   weekend dungeon crawler to a Metroid Prime-scale co-op game — can adopt CycleVania as its
   procgen engine without fighting it.
2. **Hosts own content, CycleVania owns algorithms.** The host worries about game design, content,
   and mechanics; CycleVania takes as much generation work as possible off their hands — including
   producing the level geometry itself (engine-agnostic float buffers, never engine objects).
3. **The full fidelity spectrum from one pipeline.** The same generator, fed different data,
   produces anything from a small boxy, orthogonal, lateral dungeon crawler to organic,
   vertical, landmark-rich worlds at the complexity and scale of the N64/GameCube/Wii
   *Legend of Zelda* and *Metroid Prime* titles — always 100% solvable, always deterministic.
4. **Excellent tooling, minimal host code.** The Inspector is a full dataset workbench: create,
   modify, and test registries and dials visually, play-test generated worlds, and export exactly
   what a host needs to reproduce what's on screen. Additional tooling (a headless CLI, world
   reports) is welcome wherever it has a clear separation of concerns.
5. **Every dial is dynamic.** Everything the algorithm can vary is exposed as a named,
   host-tunable dial with a sensible default — never a buried constant.
6. **No guesswork, ever.** Generated worlds are self-describing: auto-updating flowcharts of the
   mission graph, sphere ladders, dial snapshots, and **reproduction bundles** (seed + dials +
   dataset + request log) mean another developer can reproduce any displayed result in their own
   host project exactly, without reverse-engineering anything.

## The fidelity spectrum (goal 3, made concrete)

This is the single most important unification in this redesign, so it gets stated up front. Older
design layers treated "boxy dungeon crawler" and "organic Metroid Prime world" as different
generators (a cell-painting kit grammar vs. an SDF pipeline). **They are the same pipeline.** The
volumetric layer (L3, [08](./08-volumetric-composition.md)) composes every space as a signed
distance field, and the finish layer (L4, [09](./09-naturalization-and-kit.md)) meshes it with
normal-quantized dual contouring. What varies is *data*:

| Dial | Boxy crawler end | Organic epic end |
|---|---|---|
| Hull archetypes | `box` only | halls, rotundas, caverns, shafts, outdoor bowls, authored landmarks |
| Noise displacement | amplitude 0 | biome-driven fbm displacement |
| Fidelity angle step | 90° (pure orthogonal) | 5° (PS2-era faceting) or off (smooth) |
| Voxel resolution | coarse (1 cell ≈ 1 wall unit) | fine |
| Vertical budget | near-zero (lateral) | full Z spread, shafts, drops |
| Outdoor chance | 0 | biome-weighted |
| Landmarks per Reach | 0 | 1–2, visibility-biased |

Dual contouring reproduces perfectly flat, sharp-cornered boxes exactly when the hulls are boxes
and normals snap to 90° — so the "simple" end of the spectrum is not a degraded mode or a legacy
path; it is the same proven algorithm at different dial values. Shipped presets for three points on
this spectrum are specified in [14 · Dial reference & presets](./14-dial-reference-and-presets.md).

## Non-negotiable principles

Every layer, every module, every future extension must satisfy all of these. They are ordered
roughly by how catastrophic a violation would be.

1. **Solvable by construction, never by retry.** Items are placed such that the world is
   guaranteed solvable the moment placement finishes (assumed fill,
   [03](./03-mission-graph.md)). There is no "generate → check → regenerate on failure" loop
   anywhere in the design. Validation exists to catch *bugs in host data or CycleVania itself* and
   fails loudly — never to steer generation.
2. **Deterministic end-to-end.** One `(WorldSeed, ReachRequestLog)` → byte-identical output,
   forever, on every platform. No `Math.random`, no host trigonometry, forked RNG streams keyed by
   stable identity. See [02 · Determinism](./02-determinism.md).
3. **Game-agnostic by contract, not by accident.** CycleVania never knows what a capability *does*
   in gameplay terms, what a puzzle *is*, what a biome *looks like*, or what triggers a Reach to
   generate. All gameplay meaning enters through named registry contracts
   ([05](./05-capabilities-and-facets.md), [06](./06-puzzles-locks-and-recipes.md)) and host
   callbacks with strict purity requirements. The litmus test: every document in this suite is
   readable, and every algorithm exercisable in tests, without a single real gadget name, puzzle
   mechanic, or biome name appearing.
4. **Engine-agnostic geometry out.** CycleVania *produces* geometry — triangle buffers, a
   collision grid, anchors — but only as plain serializable data (`number[]` buffers + metadata).
   Core imports no engine, no DOM, no Node-only APIs. The host's realizer converts data to engine
   objects. "Renderer-free" means "no rendering dependencies," not "no geometry."
5. **Organic by construction, not by decoration.** Entropy and weighting are baked into the choice
   algorithms themselves (budget curves, schedulers, hull selection, anchor scatter) — variety is a
   property of every layer, not a paint coat at the end.
6. **Virtual space is not world space.** Worlds and Reaches are pure data containers deciding
   *what* should exist; they never carry a literal 3D transform. Coordinates first appear when the
   spatial skeleton (L2) places Spaces. A World's bounds are only ever the union of realized Area
   bounds — never pre-allocated.
7. **Lazy by default.** Nothing is generated until requested. A Reach doesn't exist, even
   abstractly, until an explicit `ReachRequest` asks for it ([04](./04-worlds-reaches-and-pacing.md)).
   Previews of unrealized content are pure, read-only, and O(1).
8. **Fail loudly, and know whose fault it is.** Malformed host data raises a typed `GenError` with
   a precise diagnostic at generation time; an internal contradiction throws (a CycleVania bug).
   Nothing is ever silently dropped, clamped away, or "best-effort"-ed
   ([12](./12-orchestration-and-host-integration.md)).
9. **One vocabulary, not parallel systems.** Tag strings are a single shared namespace used by
   capability Facets, puzzle recipes, socket signatures, and content anchors — cross-referenced and
   validated at registry-definition time so an orphaned or misspelled tag is a load-time error, not
   a silent no-op.

## Lineage — what earlier design layers contributed, and what is retired

CycleVania went through four design/implementation layers before this suite. Knowing what was
deliberately retired (and why) matters as much as knowing what was kept — an implementer finding an
older concept in prior art must not resurrect it.

| Layer (oldest → newest) | Kept in this design | Retired by this design |
|---|---|---|
| **CrawlStar's embedded procgen** (the original host; halted when its procgen proved shallow/buggy) | The Archipelago solvability model; seeded forkable RNG + deterministic trig; the horizon/streaming idea; "Omens" → generalized as Reach modifiers; teach→test→combine lock pacing; compact deterministic descriptors | Its boxy AABB layout; direct mesh emission (engine-coupled); everything hardcoded to one game's fiction |
| **Original CycleVania design** (Phase A/B; implemented and green) | The solvability core (Rule algebra, region graph, spheres, assumed fill, 1000-seed soak); the template DSL; the simulator + autosolve; async orchestration; the Inspector concept; the migration-shim strategy for hosts | The **two-tier cell grid + host-supplied per-cell GeometryKit grammar** (the "cell painter") — superseded by generated geometry; `CapabilityProfile`/`ChallengeTemplate` — superseded by Facets/PuzzleDefs |
| **Phase D geometry backend** (implemented and green) | The entire volumetric/finish pipeline: SDF hulls + spline connectors + composed area fields; 5° normal-quantized dual contouring with QEF; grid-aligned kit dedup; occupancy grid + swept-sphere collision; deterministic 3D noise; force-directed layout; the organic gadget economy | Its composer wiring (the middle layer it was bolted onto is redesigned); geometry deciding abstract facts (e.g. whether a room is outdoor — that decision moves to L2, [07](./07-spatial-skeleton.md)) |
| **Clean-sheet redesign docs** (newest prior art) | Nearly everything: the four-layer split, on-demand `ReachRequest` generation, Reach modifiers, the complexity formula, Facets, the first-class Puzzle pool, virtual schedules + final sweep, socket triple duty, connectivity degrees, Poisson content scatter, landmarks, the dial audit, `ReachPortal`, the Metroid Prime case-study fixtures | Its treatment of L3/L4 as "a recommendation" — this suite commits to the proven SDF/dual-contouring pipeline as the concrete specification, with fidelity as data (the spectrum above) |

## What CycleVania guarantees — and what it doesn't

- It guarantees every non-bonus Location is **reachable**, never that it is **survivable**.
  CycleVania has no model of player stats, gear, or difficulty-vs-power tuning — a host is free to
  build a game where a logically-reachable Reach is practically a wall until the player gets
  stronger. The contract ends at "reachable," on purpose.
- It guarantees **byte-identical reproduction** given the same seed, the same registry data, the
  same engine version, and the same request log — and it records all four in the output so the
  guarantee is checkable ([02](./02-determinism.md), [10](./10-output-contract.md)).
- It does **not** simulate gameplay: puzzle logic, resource regeneration, combat, physics beyond
  the collision grid — all host runtime concerns. CycleVania only ever consumes their *generation
  consequences* through registries.

## Glossary

| Term | Meaning |
|---|---|
| **World** | The whole generated game world: a seed, a length policy, and however many Reaches get requested. Pure data container; no transform. |
| **Reach** | One on-demand generation unit: a complete, independently-solvable chapter of the World (its own mission graph, Areas, items). |
| **Region** | A node in a Reach's mission graph (L1). Maps 1:1 onto one Area. |
| **Area** | A Region realized in space (L2): a cluster of Spaces with real coordinates, one composed geometry field, one kit. The streaming unit. |
| **Space** | One coherent volume inside an Area: a Room (enclosed), an Outdoor space (sky-exposed), or a Connector (tube/shaft between two Sockets). |
| **Socket** | One primitive doing triple duty: a structural connection point between Spaces, a WFC-style compatibility tag carrier, and a content-spawn anchor. |
| **Traversal** | How a connection is crossed: `walk · climb · crawl · drop · swim · vertical`. Carried by sockets and connectors; `drop` implies one-way unless a capability re-opens it. |
| **Location** | A placeable slot in a Region (pedestal, chest, switch) that assumed fill drops Items into. |
| **Item / Gadget** | A placeable thing. A Gadget grants one or more Capabilities. |
| **Capability** | An opaque host-defined id a progression Item grants; carries Facets describing its generation consequences. |
| **Facet** | One evaluated aspect of a Capability: a Magnitude (feeds a budget bucket), a Tag (enables tagged geometry/puzzles), or a Resource (a consumable pool hint). |
| **Puzzle / Lock** | A host-authored `PuzzleDef`: a Rule-gated challenge with an outcome, scheduled and placed like items. A Lock is the gating use of a Puzzle. |
| **Rule** | The gating algebra: `always · have · count · flag · and · or · not`. |
| **Sphere** | The solvability ladder: Sphere 0 is reachable with starting state; Sphere *n* once everything from earlier spheres is held. |
| **Flag** | A named boolean world fact set by solving something; first-class in the mission graph. |
| **Hull** | A Space's open volume as a signed distance field (negative = open/walkable, positive = solid). |
| **Kit / Piece / Instance** | The generated modular geometry: deduplicated grid-cell-local meshes (pieces) + world-grid placements (instances). |
| **Occupancy grid** | The serializable per-cell solid/open grid derived from the field; the collision representation. |
| **Reach modifier** | A player-chosen, host-defined risk/reward dial patch applied to one Reach's generation. |
| **ReachPortal** | Cross-Reach navigation topology (elevators, one-way exits) — map data, never solvability input. |
| **Dial** | Any named, host-tunable parameter. [14](./14-dial-reference-and-presets.md) lists all of them. |
| **Registry** | The validated bundle of all host-supplied data contracts. |
| **Realizer** | Host-side code that converts CycleVania's output descriptors into engine objects. |
| **Reproduction bundle** | Seed + registry + dials + request log + engine version: everything needed to reproduce a world exactly. |
