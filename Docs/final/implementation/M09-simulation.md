# M09 · Simulation & autosolve

**Goal**: the `sim/` module — the deterministic playtest reducer over abstract facts, the full
command set (including `why` and `see`), and `autosolve` as the bot-completes-a-Reach proof run
across the soak matrix.

**Required reading**: [redesign 11 (all)](../redesign/11-simulation-and-autosolve.md); the
volatile-flag rule in [redesign 06](../redesign/06-puzzles-locks-and-recipes.md).

**Prerequisites**: M08 green. (M10 does not depend on this milestone; they may be built in
either order.)

**Porting references**: legacy `core/src/sim/{state,command,parser,reducer,autosolve,world}.ts` —
the reducer/parser skeleton is proven; the projection and command set need reworking to the new
descriptor shapes.

## Phase 9.1 — Projection: `sim/world.ts`

`buildSimWorld(world: WorldDescriptor | ReachDescriptor): SimWorld` per redesign 11's shape:
unify sockets + connectors + portals + ReachPortals into one `SimLink[]` (`{ id, from, to,
traversal, gate?, oneWay? }` — a two-way link becomes two entries); project Spaces with their
anchors and bindings; index items/puzzles from the registry snapshot embedded in placement data.
No geometry anywhere in the projection.

## Phase 9.2 — State & reducer: `sim/state.ts`, `sim/reducer.ts`, `sim/parser.ts`

`SimState` exactly per redesign 11 (volatile flags in a separate set, auto-cleared when `at`
changes away from the Space that set them). `step(world, state, cmd)` — pure: clone-in,
clone-out; deterministic diegetic-ready message strings. Commands, all of redesign 11's list:

- `move` — gate-checked (`evalRule` incl. volatile if currently set); records one-ways; marks
  `visited`.
- `goto` — BFS over currently-open links (deterministic neighbor order: link insertion order),
  then apply the path as moves.
- `take` — collects the `location`-bound anchor at `at`; folds grants into `held`.
- `use` — applies the item's registry use-effect (set flag / spend resource charge).
- `interact` — if the target anchor's bound puzzle instance's `condition` passes, apply its
  outcome (set flag / open edge in the sim's link view / spawn item locally).
- `see` — gate-aware report: open links, visible-but-blocked gates (with full `missingCaps`),
  perception-gated content only when the perception tag is active.
- `why` — the full unmet set for a named link/puzzle.
- `give` / `reset` / `solve` — debug, reset, run autosolve from here.

`parseCommand("/use item-id")` — the slash-prefixed text form for REPL use.

## Phase 9.3 — `sim/autosolve.ts`

The sphere-guided greedy walker per redesign 11's pseudocode: lowest-incomplete-sphere targets →
satisfiable unsolved puzzles → the terminal link; BFS-nearest; act; loop. Returns the full action
log. **Failure throws a plain Error** (internal bug class: `isSolvable` passed but the walk
failed) with the state + missingCaps analysis embedded in the message.

## Phase 9.4 — Tests (`sim/sim.test.ts`)

- Reducer purity: `step` twice from a cloned state → identical results and messages.
- Gate behavior: blocked `move` leaves state unchanged and messages the full unmet set; after
  `take` of the required gadget, the same `move` succeeds.
- Volatile: a volatile flag set by `interact` clears on leaving the Space; a shortcut gated on it
  closes again.
- One-way: a `drop` link traverses forward, refuses reverse, and is recorded.
- `see`/`why`: perception-gated anchor invisible without the tag, visible with it; `why` lists
  exactly the unmet caps of a compound gate.
- **Autosolve soak**: 100 seeds × the synthetic registry (geometry off) — terminal reached every
  seed; plus one seed with `geometry: true` proving geometry's presence changes nothing.
- Pacing metric: the action log exposes `movesBetweenRewards` summary (used later by tooling) —
  assert it's computed and finite.

## Definition of Done

- [ ] Autosolve soak green; any divergence between `isSolvable` and `autosolve` treated as a
      blocking bug (do not weaken either side to pass).
- [ ] All sim behavior identical with and without geometry attached.
- [ ] Full root suite + typecheck green. STOP — hand off. Do not commit.

## Pitfalls

- The sim re-uses `evalRule`/`missingCaps`/`buildHeld` from M01/M04 — never reimplement rule
  logic locally (one logic implementation everywhere is the contract).
- Keep BFS neighbor order = link insertion order — path choice is part of the deterministic
  action log.
