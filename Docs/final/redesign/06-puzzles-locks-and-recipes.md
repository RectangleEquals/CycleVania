# 06 · Puzzles, Locks & spatial recipes

> Puzzles and Locks are their own **first-class content pool**, treated with exactly the machinery
> Capabilities get ([05](./05-capabilities-and-facets.md)): their own registry, economy config,
> scheduler, virtual schedule, and final sweep. A Puzzle can be as small as a single switch or as
> large as a World-spanning collectathon — and its **spatial recipe** is the bridge that turns a
> logical gate into physical space, with a hard invariant that geometry can never diverge from
> logic.

## Why Puzzles need their own pool

A template's `gating.lockFraction` says *how much* of a critical path is locked — a structural
ratio. It says nothing about *which* puzzle mechanic sits behind a lock, how many distinct puzzle
designs a host authored, how they distribute across a World, or how a puzzle's outcome can be a
reward in itself. Different question ⇒ same machinery as gadgets, second catalog.

## `PuzzleDef`

```ts
type PuzzleScope = "room" | "area" | "reach" | "world";
type PuzzleClass = "required" | "optional-reward" | "optional-shortcut" | "cosmetic";

interface PuzzleDef {
  id: string;
  scope: PuzzleScope;          // how far its condition's inputs may draw from (validation-time constraint)
  class: PuzzleClass;
  condition: Rule;             // the SAME Rule algebra as every edge (03) — composability for free
  outcome: PuzzleOutcome;
  powerWeight: (level: number) => number;      // identical scheduling shape as CapabilityDef
  guarantee?: { withinReachLevels: number };   // identical pity shape
  spatialRecipe?: string;      // names a SpatialRecipeDef (below); shared tag vocabulary
  archetype?: string;          // host-only authoring label — stored, never interpreted
  revision?: number;
}

type PuzzleOutcome =
  | { kind: "open-edge"; edge: EdgeSpec }
  | { kind: "spawn-item-here"; item: ItemSpec }
  | { kind: "unlock-adjacent-space"; spaceHint: string }
  | { kind: "grant-capability"; capability: CapabilityId }
  | { kind: "set-flag-only" }
  | { kind: "world-ending" };

interface EdgeSpec { from?: string; to: string; oneWay?: boolean; }  // region ids; from defaults to the bound Region
interface ItemSpec { itemId: string; }                               // must exist in the GadgetCatalog (registry-fixed rule, 05)
```

`condition` being ordinary Rule algebra makes "a puzzle requiring a specific gadget" free:
`and(flag("levers-aligned"), have("grapple"))` gates on the player's own solving action (a flag
only the host's runtime sets) *and* an item, in one Rule. CycleVania never simulates puzzle logic —
only its Rule and its shape hint.

## Scope

| Scope | Condition may reference | Example |
|---|---|---|
| `room` | state local to one Location/Space | a switch, a block puzzle, a lockdown room |
| `area` | flags set anywhere in one Area | a multi-room water-level puzzle; scattered switches sharing one gate |
| `reach` | anything in one Reach | collect 3 of 5 Reach-local fragments to open the capstone |
| `world` | a `count()` over capabilities/flags spanning **multiple Reaches** | the classic 12-artifact collectathon |

Scope is a **validation-time constraint**, not a separate spatial mechanism — it bounds how far
`validateGraph`/reachability may look when resolving the condition, using the same evaluation
everywhere.

## The World-scope collectathon pattern

The reference case: N fragments individually scattered across every part of the World, each behind
its own ordinary capability lock, brought to one shrine that opens the finale once all are held.

1. **The fragment is a Capability, not a Puzzle** — a facet-less `CapabilityDef` (it exists purely
   to be counted), N copies of one id, scheduled across Reaches by the ordinary gadget scheduler +
   virtual schedule, each Location gated however fill's safety allows.
2. **The shrine is a `PuzzleDef`** with `scope: "world"`, `condition: count("fragment", N)`,
   `class: "required"`, `outcome: open-edge` to the finale.
3. **A singular load-bearing set-piece is pinned by template role** (a designated
   `capstone`/`terminal` node at a chosen depth via a single-entry `ReachTemplatePool` slot) rather
   than drawn probabilistically — the scheduler exists for *numerous, interchangeable* instances;
   hand-placement by role is equally supported.
4. **Chaining**: the opened edge can lead to a second `PuzzleDef` (a `room`-scope boss,
   `condition: flag("boss-defeated")`, `outcome: world-ending`). Two ordinary defs, one ordinary
   edge — no new plumbing. The same chaining works small: `spawn-item-here` placing an item whose
   capability a second Rule on the same room's exit needs is just two Rules resolved in sequence,
   which assumed fill already handles.
5. The fragment count must satisfy the sweep guarantee: give the fragment capability
   `guarantee.withinReachLevels` ≤ the World's max length so a bounded World always completes the
   set ([04](./04-worlds-reaches-and-pacing.md)).

## The lock taxonomy — every gating scenario as a `PuzzleDef` pattern

None of these rows needs a mechanism beyond `condition`/`outcome`/`scope`/`class`:

| # | Pattern | `condition` | scope | class | notes / space-shaping |
|---|---|---|---|---|---|
| 1 | Capability lock | `have(cap)` / `count(cap, n)` | room | required | `open-edge` |
| 2 | Physical switch | `flag(name)` | room | required | trivial — a **Switch**, no recipe needed |
| 3 | Timed switch | `flag(name, { volatile: true })` | room | *optional only* | see the volatile rule below |
| 4 | All-on-at-once | `and(flag(a), flag(b), …)` | area | required | recipe `panel-array` |
| 5 | Activation order | flag-chain (each sets the next's precondition) | area | required | recipe `sequence-path` |
| 6 | Multi-room | a flag set in one Region gates an edge in another | area | required | first-class `FlagDef` provenance (03) |
| 7 | Combination | `count(flagGroup, k)` | room | required | recipe `combination-lock` |
| 8 | Arena lockdown | enter ⇒ `flag(locked)`; exit rule `flag(cleared)` | room | required | recipe `arena` |
| 9 | Environmental/interactive | host-defined, any shape | room–reach | either | recipe names a host archetype |
| 10 | Perception | `have(perceptionCap)` | room | required | geometry tagged `revealable` (08, 09) |
| 11 | Hazard field | `have(immunityCap)` | room | required | biome hazard set (08) |
| 12 | Boss gate | `flag(boss-cleared)` | room | required | lives in the capstone Area (03) |
| 13 | World collectathon | `count(scatteredCap, N)` | world | either | pattern above |
| 14 | Pure bonus/secret | anything, behind an off-path branch | room–area | optional-reward | never gates progress |
| 15 | Shortcut/loop unlock | anything | room–area | optional-shortcut | `open-edge` back toward an earlier Region |

**Switch vs. Puzzle, precisely**: a bare on/off or timed flag (#2, #3) is a Switch — trivial, no
recipe. The moment two or more flags must combine, sequence, or cross Regions (#4–#7), it graduates
to a Puzzle and carries a `spatialRecipe` so space can be shaped to fit it.

### The volatile-flag rule (hard)

A `volatile` flag must **not** be assumed permanently set when solvability is computed — otherwise
a "you had to be fast enough" route could masquerade as a structurally-guaranteed path. The
baseline proof always runs against the non-volatile subset of state; a volatile flag may only gate
`optional-shortcut`/`optional-reward` content, **never a required edge**. `defineRegistry` rejects
any `required`-class def whose condition transitively depends on a volatile flag.

## Spatial recipes — where logic becomes space

A `spatialRecipe` names a **`SpatialRecipeDef`**: a parameterized spatial requirement L2 reserves
and L3 realizes. This was left as an opaque string in prior layers; the final shape:

```ts
interface SpatialRecipeDef {
  id: string;                          // the name PuzzleDef.spatialRecipe refers to
  space: {
    kinds: ("room" | "outdoor" | "connector")[];  // which Space kinds may host it
    minExtent: [number, number, number];          // reserved envelope, world-grid cells
    preferRoles?: NodeRole[];                     // e.g. an arena prefers capstone/gate Regions
    biomeAffinity?: string[];
  };
  sockets?: { count: number; traversal: Traversal; signature: string }[];  // required structural sockets
  anchors: { kindId: string; count: { min: number; max: number }; tags?: string[] }[];
                                       // required content anchors (interactables, plates, emitters…)
  geometry?: {
    carve?: string;                    // named SDF carve-in the hull applies (e.g. "hazard-trench", "ledge-shelf")
    revealable?: boolean;              // marks produced pieces `revealable` (collision toggled by the host on the tag)
    scaleWith?: "depth" | "budget";    // how minExtent/hazard length scale
  };
  difficulty: { base: number; scaleWithDepth?: number };
}
```

Order of operations: when a scheduled Puzzle instance is bound to a Region (fill time), the
`AreaComposer` **reserves an envelope** meeting `space.minExtent` in a qualifying Space (creating
one if the drawn Space set has none — a legal budget adjustment *before* layout, step 8 of the
master sequence), registers the required sockets/anchors as constraints, and passes the recipe to
the owning `SpaceComposer`, which applies `geometry.carve` during hull composition and binds the
required anchors during scatter ([08](./08-volumetric-composition.md)).

**Shipped recipe archetypes** (data, overridable): `gap-crossing`, `high-ledge`,
`hidden-crossing`, `hazard-field`, `moving-route`, `sealed-barrier`, `powered-mechanism`,
`panel-array`, `sequence-path`, `combination-lock`, `weight-plate`, `arena`,
`collectathon-shrine`, `boss-arena`. Each ships with sensible `space`/`anchors`/`geometry`
parameters so a host maps its own puzzles onto them by id and gets generation semantics with zero
composer code.

### The coupling invariant (hard)

The Rule stamped on a gated edge **is** (never "matches") the referenced Puzzle instance's
`condition` — one object, one source of truth — so geometry and logic cannot diverge. And a
`required`-class instance may only demand capabilities guaranteed in scope for its Reach
(`startHeld` + this Reach's own placements); a recipe demanding anything else is forced to an
optional class or rejected at validation. This is what keeps an affordance-biased placement (a
high ledge sized to a jump upgrade) from ever stranding the golden path.

### Teach → test → combine

When a Reach introduces a new capability, its lock placement follows a paced sequence, controlled
by a dial:

```ts
interface LockPacingConfig { teachTestCombine: boolean;  // default true
                             combineChance: number; }    // chance later same-Reach locks compose the new cap with an older one
```

With it on, the first gate answering the new capability is a *simple* `have(cap)` in a low-stakes
recipe (teach); subsequent gates raise recipe difficulty (test); late-Reach gates may compose it
with earlier-sphere requirements via `and` (combine), at `combineChance`. Purely a bias over which
scheduled Puzzle instances bind to which edges — solvability machinery is untouched.

## Scheduling parity with gadgets

```ts
interface PuzzleEconomyConfig { min: number; max: number; }   // puzzles per Reach
```

Everything in [05](./05-capabilities-and-facets.md)'s scheduler section applies verbatim with
Puzzle substituted: a fixed seed-independent `PuzzleCatalog`; per-Reach `min/max` adjustable via a
modifier's `dials.puzzleEconomy` and `ReachRequest.puzzleEconomyOverride`; the identical
eligibility/pity mechanism on `rng.fork("${reachRoot}:puzzle-schedule")` — a stream fully
independent of the gadget stream; its own virtual schedule + final sweep (for `required`-class
entries) under the same `WorldLengthPolicy`. Two schedules, not one, because hosts legitimately
pace puzzles differently than gadgets (denser early — simple puzzles teach mechanics before
gadgets complicate them).

`LockVocabulary` — a registry of named recipe → Rule-builder pairs for the common taxonomy
patterns — is the authoring convenience layer over all of the above: hosts (and the shipped
presets) declare locks by pattern name instead of hand-assembling defs.

## Design grounding (for calibration, not prescription)

- Classic action-adventure puzzle archetypes recur far more than they vary (block, switch, torch,
  target, escort/return, enemy — a handful of patterns layered and re-dressed). Keep `archetype`
  a small reusable vocabulary; expect most authored content to be recognizable variations, with a
  few hand-authored set-pieces doing the memorable heavy lifting.
- Required puzzle density per major dungeon in the genre's classics is modest — a handful of
  required beats per Area, concentrated in capstones (the boss-key pattern). Default
  `PuzzleEconomyConfig` accordingly ([14](./14-dial-reference-and-presets.md)).
- Optional collectathons work at Area scope too (an Area-local fragment set granting a bonus),
  not just World scope — ship both scales as preset examples.
- Keys and rewards spread evenly beat clustering; long discovery droughts kill exploration. The
  placement weights ([03](./03-mission-graph.md)) encode exactly this.
