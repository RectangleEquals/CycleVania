# 08 · Case study — modeling Metroid Prime (2002) at scale

> A worked reconstruction exercise: take a real, shipped, ~100-item, ~200-room 3D metroidvania and
> show it maps onto CycleVania's schema without inventing a single new mechanism beyond what
> `Docs/redesign/00`–`07` already define. This is **not** a claim that CycleVania would
> *procedurally generate* Metroid Prime — that game is hand-authored and should stay that way. The
> claim is narrower and more useful: CycleVania's registries are **expressive enough** to hold
> content at this scale and structural complexity, which is the actual bar an original game needs
> cleared before implementation starts.

## Method and sourcing

Everything below is drawn from public strategy-guide and speedrunning-wiki documentation of the
2002 GameCube original, cross-checked against **Randovania** — the actively-maintained, open-source
randomizer platform that already models Metroid Prime as a formal logic graph for its own item
placement. Randovania's own glossary is worth quoting up front, because its vocabulary is
strikingly close to CycleVania's own, independently arrived at: <cite index="32-1">a GameDescription contains a list of Regions, a Resource Database, and a Victory Condition; Docks link directly-adjacent Areas within a Region, while Teleporters link disjointed Areas or entire Regions (elevators, in Prime's case); Events are Resources representing a one-time, non-undoable player action</cite> — i.e. Region ≈ Reach, Area ≈ a room-cluster, Dock ≈ Socket, Teleporter ≈ a cross-Reach connection, Event ≈ Flag,
Resource ≈ Capability. This is independent validation that the four-way split in
[00](./00-overview.md) isn't a CycleVania idiosyncrasy — it's close to how the community that
actually formalized this game's logic already thinks about it.

## Part 1 — the dataset

### Regions, sub-areas, and room counts

Metroid Prime ships **7 top-level regions**<cite index="17-1">, with Tallon Overworld considered the central hub, linking to every other region except Phendrana Drifts</cite>:

| Region | Sub-areas | Rooms (approx.) | Boss(es) | Notes |
|---|---|---|---|---|
| Space Pirate Frigate (Orpheon) | Decks Alpha/Beta/Gamma, Reactor Core | ~26 | Parasite Queen | Linear prologue; ship sinks after escape — **never revisited** |
| Tallon Overworld | Landing Site, Frigate Crash Site, Artifact Temple, Tallon Canyon | ~20 | Chozo Ghosts, Meta Ridley (both late-game) | The primary hub; houses the World-scope Artifact Temple |
| Chozo Ruins | Entrance/West, Central, Far/East<cite index="66-1"> — 29, 21, and 14 rooms respectively, 64 total, the largest region in the game</cite> | 64 | Hive Mecha, Incinerator Drone, Plated Beetle (minis), Flaahgra | Densest item concentration: 4 Energy Tanks, 19 Missile Expansions, 1 Power Bomb Expansion, 3 Artifacts, plus 7 major items<cite index="69-1"> including the Morph Ball, Bomb, Charge Beam, Missile Launcher, Ice Beam, Wavebuster, and Varia Suit</cite> |
| Magmoor Caverns | (no sub-division) | ~15 | *(none)* | <cite index="62-1">A connective "subway" linking every region except Impact Crater</cite> — a **secondary hub**, not just a corridor |
| Phendrana Drifts | ancient Chozo ruin, Space Pirate research lab, Phendrana's Edge<cite index="62-1">, reachable only via Magmoor Caverns</cite> | ~30 | Sheegoth (mini), Thardus | |
| Phazon Mines | Level One/Two/Three<cite index="17-1"> — entrance, mining/research, and Phazon-and-fungus zones</cite> | ~20 | Elite Pirates ×3, Cloaked Drone, Phazon Elite (minis), Omega Pirate | |
| Impact Crater | small, linear | ~5 | Metroid Prime (final, 2-phase) | Only reachable after the World-scope Artifact condition is met |

### The item pool (100 total, exactly matching what a randomizer shuffles)

<cite index="21-1">A community item randomizer for this game shuffles the locations of all 100 items</cite>, decomposing as:

| Category | Count | Examples |
|---|---|---|
| Major/unique capabilities | ~22 | Morph Ball, Bomb, Boost Ball, Spider Ball, Power Bomb, Charge Beam, Wave/Ice/Plasma Beam, Super Missile, Wavebuster, Ice Spreader, Flamethrower, Missile Launcher, Grapple Beam, Space Jump Boots, Varia/Gravity/Phazon Suit, Scan/Thermal/X-Ray Visor |
| Energy Tanks | <cite index="42-1">14</cite> | +100 max energy each, progressive |
| Missile Expansions | <cite index="39-1">49</cite> | +5 missile capacity each, progressive, non-gating |
| Power Bomb Expansions | <cite index="39-1">4</cite> | +2 Power Bomb capacity each |
| Chozo Artifacts | 12 | the World-scope collectathon, gates the final Reach |

### Beam combos — a real, shipped "Combo capability"

<cite index="40-1">Reaching maximum capacity requires all four Charge Beam combos — Super Missile, Wavebuster, Ice Spreader, and Flamethrower — in addition to every Energy Tank</cite>. Each combo requires **Charge Beam held simultaneously with** the matching weapon (Missile/Wave/Ice/Plasma) — this is a shipped game using exactly the `held: { derivedFrom: [...] }` combo pattern from [03](./03-locks-keys-and-gadgets.md), independently of CycleVania.

### Bosses as multi-step puzzles, not just health bars

- **Flaahgra** (Chozo Ruins, reward: Varia Suit) — <cite index="50-1">stun it, then hit the weak points on the surrounding mirror dishes with missiles or charged shots to flip them up; once all are raised, it falls and can be finished with a Morph Ball bomb</cite>. A genuine multi-condition `PuzzleDef`, not a single flag.
- **Thardus** (Phendrana Drifts, reward: Spider Ball) — <cite index="54-1">requires switching to the Thermal Visor to expose weak points, alternating visors throughout the fight</cite>. A capability-gated boss puzzle (`have("thermal-visor")` as a precondition of the fight even being winnable).
- **Omega Pirate** (Phazon Mines, reward: Phazon Suit) — <cite index="49-1">an invisible, regenerating Elite Pirate whose four armor pieces must each be individually destroyed</cite>. A `count()`-style multi-part condition.
- **The Artifact Temple → Chozo Ghosts → Meta Ridley chain** (Tallon Overworld) — the canonical **world-scope** case: collect all 12 Artifacts scattered across every region, return to the Temple, fight through Chozo Ghosts, get ambushed by Meta Ridley, and only then can the Impact Crater be reached.

## Part 2 — the CycleVania mapping

### Hierarchy correspondence

| Metroid Prime | CycleVania |
|---|---|
| Planet Tallon IV | World |
| a top-level Region (Chozo Ruins, etc.) | Reach |
| a Region's named sub-area (Chozo Ruins' "Far" wing) | Area |
| an individual room | Space |
| an item pickup | Item / Capability grant |
| a Dock (in-Region door) | structural Socket |
| a Teleporter (elevator, cross-Region) | **Reach Portal** — see below, a genuine gap this exercise surfaces |
| a scan-only lore/hint | a `content` Socket of kind `dressing`, no gameplay tie-in |

### `WorldLengthPolicy` for a reconstruction, vs. a generative World

This is a **fixed-content reconstruction**, not a fresh generation — so `WorldLengthPolicy =
{ min: 7, max: 7 }` (Frigate + 6 core regions), a degenerate range exactly like
`AreaCountConfig = {5,5}`'s default, just asserting "this specific World has exactly this many
Reaches" rather than leaving it open. A *new*, CycleVania-generated game inspired by this scale
would instead use a real range (see [06](./06-dial-audit.md)'s calibration philosophy) — the fixed
policy here is specifically because we're encoding an *existing, already-designed* sequence, not
generating a new one.

### `AreaCountConfig` has to flex hard, region by region

Chozo Ruins alone (64 rooms across 3 sub-areas) blows straight past the `{min:5, max:5}` default
from [02](./02-composers-and-complexity.md) — which is exactly the point of that dial being a
config, not a constant. A faithful per-Reach override:

| Reach | `AreaCountConfig` override | Spaces per Area (approx.) |
|---|---|---|
| Frigate Orpheon | `{min:4, max:4}` (Alpha/Beta/Gamma decks + Reactor Core) | ~6–7 |
| Tallon Overworld | `{min:4, max:5}` | ~4–5 |
| Chozo Ruins | `{min:3, max:3}` (Entrance/Central/Far) | ~10–21 — itself wildly uneven, matching [02](./02-composers-and-complexity.md)'s budget-pooling note, not a bug to fix |
| Magmoor Caverns | `{min:1, max:2}` | ~15 (mostly one long connective wing) |
| Phendrana Drifts | `{min:3, max:3}` (ruin/lab/edge) | ~10 |
| Phazon Mines | `{min:3, max:3}` (Levels One/Two/Three) | ~7 |
| Impact Crater | `{min:1, max:1}` | ~5 |

## Part 3 — worked `GadgetCatalog` excerpt

```ts
const spaceJumpBoots: CapabilityDef = {
  id: "space-jump-boots", held: "granted", powerWeight: () => 0.7,
  facets: [{ kind: "magnitude", bucket: "traversal.zUp", evaluate: () => 2 * JUMP_UNIT }],
  // the exact "double-jump" worked example from 03, now with a real shipped name attached
};

const grappleBeam: CapabilityDef = {
  id: "grapple-beam", held: "granted", powerWeight: () => 0.5,
  facets: [
    { kind: "tag", tag: "grapple-point" },
    { kind: "magnitude", bucket: "traversal.xyGap", evaluate: () => GRAPPLE_SPAN },
  ],
};

const powerBomb: CapabilityDef = {
  id: "power-bomb", held: "granted", powerWeight: () => 0.6,
  facets: [
    { kind: "tag", tag: "destroy-power-bomb-block" },
    { kind: "resource", poolId: "power-bomb-charge",
      capacity: ctx => 4 + ctx.level,          // base 4, +1 per Power Bomb Expansion (progressive!)
      regenHint: "site" },                     // ammo-refill Sockets scattered per 04
  ],
};

const chargeBeam: CapabilityDef = { id: "charge-beam", held: "granted", powerWeight: () => 0.3, facets: [] };
const waveBeam: CapabilityDef = { id: "wave-beam", held: "granted", powerWeight: () => 0.3,
  facets: [{ kind: "tag", tag: "open-wave-door" }] };

const wavebuster: CapabilityDef = {
  id: "wavebuster",                             // the shipped combo, modeled exactly per 03's Combo pattern
  held: { derivedFrom: ["charge-beam", "wave-beam"] },
  facets: [{ kind: "magnitude", bucket: "challenge.offense", evaluate: () => 0.7 }],
  powerWeight: () => 0.5,
};

const energyTank: CapabilityDef = {
  id: "energy-tank", held: "granted", powerWeight: () => 0.2,
  facets: [{ kind: "magnitude", bucket: "custom.survivability", evaluate: ctx => ctx.level * 100 }],
  // 14 grant-events of the SAME id — pure progressive-upgrade accumulation, no new mechanism
};
```

`GadgetDef.grants` handles the "one pickup, several independently-nameable effects" case directly
with the Varia/Gravity/Phazon Suits, which each bundle a hazard-immunity Tag *and* a
`challenge.defense` Magnitude — Option A bundling from [03](./03-locks-keys-and-gadgets.md), since
in the shipped game these effects are never independently gated.

## Part 4 — worked `PuzzleCatalog` excerpt

```ts
const flaahgra: PuzzleDef = {
  id: "flaahgra-fight", scope: "room", class: "required",
  condition: flag("flaahgra-mirrors-aligned"),   // host's own boss-state machine sets this
  outcome: { kind: "grant-capability", capability: "varia-suit" },
  spatialRecipe: "boss-arena-with-reflectors", powerWeight: () => 0.6,
  archetype: "boss-puzzle-hybrid",
};

const thardus: PuzzleDef = {
  id: "thardus-fight", scope: "room", class: "required",
  condition: and(flag("thardus-defeated"), have("thermal-visor")),  // capability-gated boss, per 03
  outcome: { kind: "grant-capability", capability: "spider-ball" },
  spatialRecipe: "arena", powerWeight: () => 0.65,
};

const artifactTemple: PuzzleDef = {
  id: "artifact-temple-unlock", scope: "world", class: "required",
  condition: count("chozo-artifact", 12),        // the world-scope collectathon pattern from 07
  outcome: { kind: "open-edge", edge: { to: "meta-ridley-encounter" } },
  spatialRecipe: "world-collectathon-shrine", powerWeight: () => 0.95,
  guarantee: { withinReachLevels: 6 },           // must resolve by the World's declared end — see 07
};

const metaRidleyChain: PuzzleDef = {
  id: "meta-ridley-fight", scope: "room", class: "required",
  condition: flag("meta-ridley-defeated"),
  outcome: { kind: "open-edge", edge: { to: "impact-crater-entry" } },  // chains directly off artifactTemple
  spatialRecipe: "boss-arena", powerWeight: () => 0.9,
};
```

The `chozo-artifact` Capability itself is a facet-less `CapabilityDef` (pure key, per
[07](./07-puzzles-and-challenges.md)'s collectathon pattern), scheduled across Reaches 1–6 by the
ordinary Gadget scheduler — 12 copies of one id, exactly like Energy Tanks, just gating a `count()`
Puzzle instead of a raw stat.

## Part 5 — the hub topology, and a genuine gap this exercise surfaces

Tallon Overworld and Magmoor Caverns are both **persistently re-traversable hubs** — the player
returns through them constantly, often with new capabilities that unlock previously-inaccessible
Locations back in an earlier Reach (a Spider Ball track in Tallon Overworld, say). Building this
concretely exposes something [01](./01-mission-graph.md)–[07](./07-puzzles-and-challenges.md)
don't yet formally name: **cross-Reach navigation topology is not the same thing as cross-Reach
solvability**, and needs its own lightweight concept.

Proposed fix, consistent with everything already established:

```ts
interface ReachPortal {
  fromReach: number; toReach: number;
  oneWay: boolean;          // Frigate's escape: true, never revisited. Ordinary hub elevators: false.
  fromSpaceHint: string; toSpaceHint: string;   // which Space in each Reach hosts the physical connection
}
```

`WorldComposer` maintains a `ReachPortal[]` list **separate from any single Reach's own
`MissionGraph`** — this is pure navigation/map data for the host's overlay, not something
`assumedFill` or `validateGraph` ever reasons about, which is exactly why it doesn't threaten the
"solvability is scoped to one Reach" guarantee from [01](./01-mission-graph.md): a portal only
describes *that two already-realized Reaches are physically connected*, never *that reaching one
requires anything from the other*. This is the concrete mechanism behind Frigate Orpheon's `oneWay:
true` (the ship sinks — no portal survives) versus every hub Reach's `oneWay: false`.

**Sequence-broken "leftover item" Locations** (a late-game capability opening an early-Reach bonus)
are already fully supported without change: they're simply `class: "optional-reward"` Locations in
an early Reach's own graph, gated by a Capability the *scheduler* happens to place later — never
required for that Reach's own solvability proof, exactly the escape hatch
[01](./01-mission-graph.md) already describes.

This gap (formalize `ReachPortal` as a first-class concept) is worth folding back into
[01](./01-mission-graph.md) or [02](./02-composers-and-complexity.md) properly rather than leaving
it here — flagging it plainly rather than pretending the existing docs already covered it.

## Part 6 — what this does *not* prove

- It doesn't prove CycleVania would generate a level as *well-crafted* as Metroid Prime — that's a
  tuning/authoring quality question, not a schema-expressiveness one, and no data model answers it.
- It doesn't exercise the *organic/volumetric* geometry pipeline ([04](./04-spatial-composition-and-sockets.md))
  against real room shapes — this case study is a mission-graph/registry-level proof, not a
  geometry-fidelity one.
- Room-count figures above are sourced from community documentation, not the original game's
  internal data, and are approximate for regions other than Chozo Ruins (where an exact, cited count
  exists).

## Part 7 — the test dataset and the actual proof

Concrete deliverables an implementer would build directly from this doc, and what each proves
against the validation strategy already defined in [05](./05-determinism-and-extensibility.md):

| Fixture file | Contents | What it lets you assert |
|---|---|---|
| `metroid-prime.gadgets.json` | All ~22 major `CapabilityDef`s + `energy-tank`/`missile-expansion`/`power-bomb-expansion` as progressive fillers + `chozo-artifact` ×12 | Facet/combo/resource/progressive-level machinery all exercised by *shipped*, not invented, mechanics |
| `metroid-prime.puzzles.json` | Every boss + colored-door + bomb-slot pattern, including the Artifact Temple/Meta Ridley chain | Every row of [07](./07-puzzles-and-challenges.md)'s taxonomy has at least one real instance |
| `metroid-prime.reaches.json` | `ReachTemplate` + `AreaCountConfig` override per region from Part 2 | `AreaCountConfig`'s range actually has to flex to 64 rooms in one Reach — a real stress test, not a synthetic one |
| `metroid-prime.portals.json` | The `ReachPortal[]` list from Part 5 | Bidirectional hub navigation and one-way prologue closure are both structurally distinct and correctly typed |
| `metroid-prime.biomes.json` | One `BiomePack` per region (verdant/Tallon, ancient-stone/Chozo, magma/Magmoor, ice-tech/Phendrana, phazon-industrial/Mines) | [04](./04-spatial-composition-and-sockets.md)'s biome contract covers real, varied aesthetic registers |

**The actual proof** is running [05](./05-determinism-and-extensibility.md)'s existing validation
strategy against this specific, fixed dataset rather than synthetic random registries: solvability
soak (does `validateGraph`/`assumedFill` accept Chozo Ruins' real 64-room, 3-Area structure and its
real item gating?), determinism golden runs, the pity/guarantee soak specifically on
`chozo-artifact`'s `guarantee.withinReachLevels: 6` (does it always resolve by Reach 6, matching
that the game is always completable?), and content-anchor overlap checks against Chozo Ruins'
real, unusually dense room layout. A dataset this close to a shipped, beloved, exhaustively-analyzed
game is a far more convincing regression suite than any amount of synthetic fuzzing — if CycleVania
can hold Metroid Prime's real complexity without a schema change, that's the "definitive proof" this
doc set out to build.
