# 11 · Simulation & autosolve (the deterministic playtest engine)

> A pure, deterministic reducer that walks a generated world **like a player** — move, pick up,
> use, interact — over the abstract facts only (graph, sockets, anchors, gates; never geometry).
> It powers the Inspector's Play mode and REPL, the `autosolve` proof that a bot can complete
> every generated Reach, and any host feature that needs "what can the player do from here?"
> (hint systems, bot companions, accessibility aids).

## Why a simulator is part of the engine

Solvability proofs ([03](./03-mission-graph.md)) prove reachability *exists*; the simulator proves
it is *walkable as an actual sequence of player actions* — and provides the vocabulary
("go here, use this, now this opens") that tooling, tutorials, and hints are built from. Keeping
it inside CycleVania (not the Inspector) means hosts get it for free and the
bot-completes-a-Reach test is a first-class engine guarantee.

## The projection: `SimWorld`

`buildSimWorld(descriptor)` projects a realized Reach (or whole World) into a lean navigable
model:

```ts
interface SimWorld {
  spaces: SimSpace[];                 // id, areaId, regionId, kind, anchors (with bindings), sockets
  links: SimLink[];                   // from/to space, traversal, gate?, oneWay?  (sockets+connectors+portals, unified)
  items: Map<string, SimItemInfo>;    // itemId → grants, class, use-effects (from the registry)
  puzzles: Map<string, SimPuzzleInfo>;// instanceId → condition, outcome, scope
  start: string;                      // the entry Space id
  spheres: string[][];
}
```

Geometry never enters the projection. (Inspector Play-mode *camera collision* uses the occupancy
grid, but that is presentation — simulation correctness is defined purely on `SimWorld`.)

## State & commands

```ts
interface SimState {
  at: string;                          // current Space id
  held: HeldData;                      // capabilities with counts + flags (volatile flags tracked separately)
  inventory: Map<string, number>;
  collected: Set<string>;              // locationIds
  solvedPuzzles: Set<string>;
  openedOneWays: Set<string>;
  visited: Set<string>;                // Space ids — drives Play-mode cascading visibility (13)
  log: string[];
}

type Command =
  | { k: "move"; link: string }        // traverse a link (gate-checked; records one-ways; marks visited)
  | { k: "goto"; spaceId: string }     // pathfind via currently-open links (BFS), then move along it
  | { k: "take" }                      // collect the anchor-bound item at the current Space
  | { k: "use"; itemId: string }       // apply the item's registry use-effect (set flag / spend charge)
  | { k: "interact"; anchorId: string }// host-shaped generic action; may set a puzzle's flag when its condition allows
  | { k: "see" }                       // report what's visible/knowable from here, gate-aware (13)
  | { k: "why"; target: string }       // explain a blocked link/puzzle: the FULL unmet set via missingCaps
  | { k: "give"; cap: CapabilityId }   // debug/testing only
  | { k: "solve" }                     // run autosolve from the current state
  | { k: "reset" };

step(world: SimWorld, state: SimState, cmd: Command): { state: SimState; ok: boolean; message: string }
```

`step` is a pure reducer: same inputs ⇒ same outputs; messages are deterministic, diegetic-ready
strings the Inspector displays verbatim and hosts may re-skin. Rule evaluation, `missingCaps`,
counted keys, derived capabilities, and volatile-flag semantics are the same code paths the
generator used — one logic implementation, everywhere ([06](./06-puzzles-locks-and-recipes.md)'s
volatile rule appears here as: volatile flags auto-clear when leaving the Space that set them).

## `autosolve` — the bot-completes-a-Reach proof

```
state ← initSim(world)
loop:
  sphere-guided greedy walk:
    targets ← uncollected reachable Locations of the LOWEST sphere index still incomplete,
              then satisfiable-but-unsolved puzzles, then the terminal link
    if none: FAIL (emit the full state + missingCaps analysis — this is a generator bug, since
             isSolvable already passed; the pair of proofs disagreeing is the highest-value
             signal this test exists to catch)
    goto nearest target (BFS over currently-open links); take / interact / use as bound
until the terminal Space is reached
return the full action log (a REPLAYABLE walkthrough)
```

Run across every soak seed ([15](./15-verification-and-test-strategy.md)); the returned log
doubles as tooling gold: the Inspector can animate it, and a diff of two seeds' logs is a pacing
diagnostic (how many moves between rewards — the "discovery drought" metric,
[06](./06-puzzles-locks-and-recipes.md)).

## Host reuse

- **Hint systems**: `why` + sphere data = an exact, spoiler-graded hint ladder ("something in the
  amber-lit area still needs what you don't have" → … → the precise missing capability).
- **Bot companions / co-op fill-ins**: `autosolve`'s greedy walker seeded with a different target
  policy is a competent navigator over real generated worlds.
- **Remembered locks**: every blocked gate the player has *seen* (`see`) with its full unmet set
  — the journal/map-pin feature — falls out of `missingCaps` + the visited set.
- **Accessibility**: "what can I do right now?" is one `reachableLocations` call on live state.
