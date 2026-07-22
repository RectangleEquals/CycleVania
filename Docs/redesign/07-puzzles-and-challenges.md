# 07 · Puzzles & challenges (game-agnostic, and just as important as Gadgets)

> Puzzles and Locks get the **identical** first-class treatment as Capabilities
> ([03](./03-locks-keys-and-gadgets.md)): their own registry, their own economy config, their own
> scheduler, their own virtual schedule and final-Reach sweep. A Puzzle is not "just another kind
> of Lock" relegated to whatever a `ReachTemplate`'s `gating.lockFraction` happens to produce — it
> is data-driven, host-authored content that can be as simple as a single flag or as large as a
> World-spanning collectathon, and CycleVania paces it with exactly the same care it paces Gadgets.

## Why Puzzles need their own pool, not just template gating

A `ReachTemplate`'s `gating.lockFraction`/`compoundChance` ([01](./01-mission-graph.md)) describes
*how much of a Reach's critical path is locked* — a structural ratio. It says nothing about *which*
puzzle mechanic sits behind a given lock, how many distinct puzzle **designs** a host has authored,
how those designs should be distributed across a whole World, or how a puzzle's own outcome might
be a reward in its own right rather than just a gate. That's a different question, and it deserves
the same machinery Capabilities already have: a registry of authored `PuzzleDef`s, scheduled across
Reaches by `powerWeight`/`guarantee`, exactly like Gadgets.

## `PuzzleDef` — the shape

```ts
type PuzzleScope = "room" | "area" | "reach" | "world";
type PuzzleClass = "required" | "optional-reward" | "optional-shortcut" | "cosmetic";

interface PuzzleDef {
  id: string;
  scope: PuzzleScope;        // how far its condition's inputs may be drawn from — see below
  class: PuzzleClass;
  condition: Rule;           // the *identical* Rule algebra as everything else in CycleVania —
                             // have/count/flag/and/or/not (see 01) — this is what makes a Puzzle
                             // composable with Gadgets for free: `and(flag("switch-hit"), have("grapple"))`
                             // is a completely ordinary condition.
  outcome: PuzzleOutcome;
  powerWeight: (level: number) => number;    // identical scheduling shape as CapabilityDef (03)
  guarantee?: { withinReachLevels: number }; // identical pity-window shape as CapabilityDef (03)
  spatialRecipe?: string;    // shared vocabulary with TagFacet.tag / Socket.signature (03, 04)
  archetype?: string;        // free-form, host-only authoring label (e.g. "block-puzzle",
                             // "torch-puzzle", "collectathon") — CycleVania stores, never
                             // interprets, same non-interpretation pattern as CapabilityDef.category
}

type PuzzleOutcome =
  | { kind: "open-edge"; edge: EdgeSpec }
  | { kind: "spawn-item-here"; item: ItemSpec }
  | { kind: "unlock-adjacent-space"; spaceHint: string }
  | { kind: "grant-capability"; capability: CapabilityId }
  | { kind: "set-flag-only" }
  | { kind: "world-ending" };
```

`condition` being the same `Rule` algebra as every edge in the mission graph is what makes "a puzzle
that requires a specific Gadget to solve" free, not a special case: a puzzle whose `condition` is
`and(flag("levers-aligned"), have("grapple"))` is gated on the player's own puzzle-solving action
(a flag only the host's runtime sets) **and** an item, in one ordinary Rule.

## Scope — where a puzzle's condition is allowed to draw from

This is the direct answer to "puzzles can span a room, an Area, or the whole World":

| Scope | Condition may reference | Example |
|---|---|---|
| `room` | flags/state local to one Location | a single switch, a block puzzle, a lockdown-until-cleared room |
| `area` | flags set anywhere in one Area (already structurally supported — Lock kind #6, below) | a multi-room water-level puzzle, several switches across a dungeon that share one gate |
| `reach` | flags/capabilities from anywhere in one Reach | collect 3 of 5 Reach-local fragments to open the Reach's capstone door |
| `world` | a `count()` over a Capability/flag-group scattered across **multiple Reaches** | the Chozo Artifact pattern, below |

Scope is a **validation-time constraint**, not a separate spatial mechanism — it only changes how
far `validateGraph`/`reachableRegions` ([01](./01-mission-graph.md)) is allowed to look when
resolving `condition`, reusing the exact same Rule evaluation everywhere.

## The World-scope collectathon pattern

The Metroid Prime Artifact Temple is the reference case: **12 Chozo Artifacts, individually
scattered across every region of the planet**<cite index="17-1">, each hidden in locations reachable only with specific items tied to the Chozo</cite>, each one <cite index="20-1">gated behind a particular movement upgrade — the Missile Launcher, Space Jump Boots, Plasma Beam, and so on</cite> — brought back to one specific room that <cite index="14-1">unlocks once all twelve are held, opening the path to the game's final confrontation</cite>. Modeled directly:

1. **The scattered fragment is a Capability, not a Puzzle** — a `CapabilityDef` with `facets: []`
   (it grants no traversal/tag/resource effect at all; it exists purely to be counted), scheduled
   across Reaches via the ordinary Gadget scheduler and virtual schedule
   ([03](./03-locks-keys-and-gadgets.md)) exactly like any other Capability — each individual
   Location gating one artifact can itself be behind an ordinary Capability lock (that's what
   ties each artifact to "an item spiritually linked" — a ordinary `have(cap)` Rule).
2. **The Temple's own puzzle is a `PuzzleDef` with `scope: "world"`**, `condition: count("chozo-artifact", 12)`,
   `class: "required"`, `outcome: { kind: "open-edge", edge: ... }` unlocking the path onward.
3. **The Temple's *own room* is typically pinned to a specific template role** (a World-defining
   set-piece like this is usually authored as a designated `capstone`/`terminal` node in a
   `ReachTemplate`, [01](./01-mission-graph.md), rather than drawn probabilistically — the
   scheduler is built for *numerous, interchangeable* puzzle instances; a singular, load-bearing
   set-piece is just as validly hand-placed by template role. Both are fully supported; this is a
   host authoring choice, not a limitation.
4. **Chaining**: opening the Temple's edge can itself lead to a second `PuzzleDef` (a
   `scope: "room"` boss fight, `condition: flag("boss-cleared")`, `outcome: "world-ending"` or
   `open-edge` to the next Reach) — exactly the "world-scale lock unlocking a new boss-fight lock"
   structure. Nothing here needs new plumbing: it's two ordinary `PuzzleDef`s, chained by an
   ordinary edge.

## Outcome chaining, in the small

The same chaining works at room scale: `outcome: { kind: "spawn-item-here", item: ... }` places an
Item that becomes part of `Held` the moment it's collected — if that Item's Capability is itself
referenced by a *second* Rule on the same room's exit edge, you get exactly "a chest spawns in this
room granting a progression item required, in this same room, to reach the exit." No special case:
it's just two ordinary Rules resolved in the same Location, in sequence, which `assumedFill`
([01](./01-mission-graph.md)) already handles.

## Scheduling parity with Gadgets

```ts
interface PuzzleEconomyConfig { min: number; max: number; }   // identical shape to GadgetEconomyConfig (03)
```

Everything in [03](./03-locks-keys-and-gadgets.md)'s "Progression-item frequency" section applies
here verbatim, substituting Puzzle for Gadget: a `PuzzleCatalog` registry (every `PuzzleDef` a host
could ever place, fixed and seed-independent, for the same determinism reason `GadgetCatalog` is);
a `PuzzleEconomyConfig.min/max` per Reach (adjustable via its own `DialPatch`-style modifier delta
and its own `ReachRequest` override); the identical eligibility/pity mechanism using
`rng.fork("puzzle-schedule")` — a stream entirely independent of `"gadget-schedule"`, so the two
pools' entropy never correlates; and its own virtual schedule + final-Reach sweep
([02](./02-composers-and-complexity.md)) under the *same* `WorldLengthPolicy`, run as an
independent pass alongside the Gadget one. A World with 24 Capabilities and 64 Puzzle instances to
place across a 4–6 Reach `WorldLengthPolicy` runs **two** virtual schedules, not one, each with its
own pacing curve, because a host may reasonably want Puzzles paced differently than Gadgets (e.g.
denser early, since simple puzzles teach mechanics before Gadgets arrive to complicate them — see
the design grounding below).

## Lock taxonomy — every scenario the brief calls out, as `PuzzleDef` patterns

Every row below is simply a common `PuzzleDef` shape — none of them need a mechanism beyond
`condition`/`outcome`/`scope`/`class` above:

| # | Pattern | `condition` | `scope` | `class` | `outcome` / space-shaping |
|---|---|---|---|---|---|
| 1 | **Capability lock** | `have(cap)` / `count(cap, n)` | room | required | `open-edge` |
| 2 | **Physical switch** (on/off) | `flag(name)` | room | required | `open-edge` |
| 3 | **Timed switch** | `flag(name, { volatile: true })` | room | required | see "Timed switches," below |
| 4 | **All-on-at-once** | `and(flag(a), flag(b), …)` | area | required | `spatialRecipe: "panel-array"` |
| 5 | **Activation order** | a flag-chain, each sets the next's precondition | area | required | `spatialRecipe: "sequence-path"` |
| 6 | **Multi-room** | a flag set in one Region gates an edge in another | area | required | flags are first-class graph facts — see below |
| 7 | **Bitmask/combination** | `count(flagGroup, k)` or a pattern-rule extension | room | required | `spatialRecipe: "combination-lock"` |
| 8 | **Arena lockdown** | enter → `flag(locked)`; exit `rule = flag(cleared)` | room | required | `spatialRecipe: "arena"` |
| 9 | **Environmental/interactive** | host-defined `condition`, any shape | room–reach | required or optional | `spatialRecipe` names a host archetype |
| 10 | **Perception** | `have(perceptionCap)` | room | required | geometry tagged `revealable` |
| 11 | **Hazard field** | `have(immunityCap)` | room | required | region tagged `hazard: <name>` |
| 12 | **Boss gate** | `flag(boss-cleared)` | room | required | `capstone` Area ([01](./01-mission-graph.md)) |
| 13 | **World collectathon** | `count(scatteredCap, N)` | **world** | required or optional | see above |
| 14 | **Pure bonus/secret** | anything, often `always` behind an off-path branch | room–area | **optional-reward** | `spawn-item-here`, `class` never gates progress |
| 15 | **Shortcut/loop unlock** | anything | room–area | **optional-shortcut** | `open-edge` back toward an earlier Region |

**Switches vs. puzzles, precisely**: a bare on/off or timed flag (#2, #3) is a **Switch** — trivial,
no `spatialRecipe` needed. The moment two or more flags must combine, sequence, or cross Regions
(#4–#7), it graduates to a **Puzzle Switch** and carries a `spatialRecipe` so `SpaceComposer` can
actually shape a room to fit it. CycleVania never simulates the puzzle logic itself — only its
`Rule` and its shape hint.

### Multi-room flags need first-class graph support

Because pattern #6 requires a flag set in one Region to gate an edge in *another*, `Flag` cannot be
edge-local — it must be a named fact on the `MissionGraph` itself (alongside `Region` and `Edge`),
with its own `setBy: LocationId | PuzzleId` provenance. `reachableRegions`
([01](./01-mission-graph.md)) already evaluates `Rule`s against a `Held` that includes flags, so
this costs nothing extra at the BFS layer — it only changes *where* a flag's origin is recorded.

### Timed switches and solvability

A `volatile` flag must **not** be assumed permanently set when `validateGraph`/`isSolvable`
computes the baseline guarantee — otherwise a route that only exists "if you're fast enough" could
be mistaken for a structurally-guaranteed path. CycleVania's solvability proof always runs against
the **non-volatile** subset of state; a volatile flag may only ever gate an **optional** shortcut or
bonus (`class: "optional-shortcut"`/`"optional-reward"`), never a required edge on the critical path.

## `ChallengeTemplate` is superseded

An earlier revision's `ChallengeTemplate { id, solvedBy, spatialRecipe, sideEffects }` is now just
`PuzzleDef` — `solvedBy` is `condition`, `sideEffects.opensEdge` is `outcome: { kind: "open-edge" }`,
`spatialRecipe` is unchanged. Nothing is lost; the shape is simply promoted to a full, schedulable
registry entry instead of a per-Lock-instance sketch.

## Grounding in real design: puzzle types, frequency, and complexity

Since "reasonable defaults" for how much puzzle content to author and how to spread it deserves
more than a guess, a few concrete reference points from well-known 3D metroidvania-adjacent design:

- **Puzzle archetypes recur far more than they vary.** Zelda's own internal taxonomy names six
  common patterns — <cite index="30-1">Block, Enemy, Return, Switch, Target, and Torch puzzles are the most frequently repeated types across the series</cite> — with more elaborate dungeons layering several
  of these together (a water-level dungeon might combine switch, target, and traversal-state
  puzzles across many rooms<cite index="31-1">, often paired with a complicating element like enemies, difficult terrain, or a required activation order</cite>). This validates keeping `archetype` a small, reusable,
  host-defined vocabulary rather than expecting every `PuzzleDef` to be bespoke — most authored
  content should be recognizable variations on a handful of patterns, with a few standout
  hand-authored set-pieces (the World-scope pattern above) doing the memorable heavy lifting.
- **Required puzzle density per major Area, historically, is modest.** <cite index="7-1">Ocarina of Time ships nine main dungeons and three mini-dungeons</cite>, each with roughly a
  handful of required lock/key beats (a small-key gate or two, a Boss Key puzzle immediately before
  the boss). This suggests a `PuzzleEconomyConfig` default on the order of a few `required` puzzles
  per Area, concentrated more heavily in `capstone` Areas (where the Boss Key pattern lives) than
  ordinary ones.
- **Optional collectathons work at Area scope too, not just World scope.** <cite index="11-1">Majora's Mask dungeons contain optional puzzles that award collectible Stray Fairies, which grant additional abilities once all are gathered</cite> — an
  Area-scoped mirror of the Chozo Artifact pattern, `class: "optional-reward"` rather than
  `"required"`. Worth shipping both scales as illustrative defaults, not just the World-scope one.
- **Keys and rewards should be spread evenly, not clustered**, and every Area should feel
  meaningful rather than padded<cite index="21-1">, since players who go too long without a rewarding discovery tend to lose interest in continued exploration</cite> — the same design principle
  `GadgetEconomyConfig`'s even-pacing philosophy already encodes, directly justifying giving
  `PuzzleEconomyConfig` the identical ranged/paced treatment rather than a simpler flat count.
- **The lock-and-key structure of these games is well-studied and maps directly onto a mission
  graph** — <cite index="24-1">Mark Brown's "Boss Keys" series analyzes each Metroid and Zelda game's progression as a dependency graph of items and the locks they open</cite>, which is
  effectively a hand-drawn version of exactly what `MissionGraph` ([01](./01-mission-graph.md))
  automates — a useful sanity check that the graph abstraction matches how designers already think
  about these games, not just a CycleVania-specific invention.

None of these are prescriptive numbers CycleVania should hardcode — they're reference points for
picking `PuzzleEconomyConfig`/`GadgetEconomyConfig` defaults, the same calibration-philosophy
approach as [06 · Dial audit](./06-dial-audit.md)'s Reach/Area scale targets.

## Registries a host supplies

| Registry | Shape | Consumed by |
|---|---|---|
| `PuzzleCatalog` | `PuzzleDef[]` | scheduler (this doc), `AreaComposer` space-shaping ([04](./04-spatial-composition-and-sockets.md)) |
| `PuzzleEconomyConfig` | `{ min, max }` puzzles per Reach | scheduler (this doc) — adjustable via a modifier's `dials.puzzleEconomy` or `ReachRequest.puzzleEconomyOverride` (extending [02](./02-composers-and-complexity.md)'s `DialPatch`/`ReachRequest` shapes) |
| `LockVocabulary` | named `PuzzleDef` recipes → `Rule` builders, for common patterns from the taxonomy above | `ReachComposer`/`AreaComposer` when authoring new Puzzle instances |
