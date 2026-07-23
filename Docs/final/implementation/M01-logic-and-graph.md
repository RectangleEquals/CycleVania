# M01 · Logic & mission graph

**Goal**: the complete L1 logic layer — the `Rule` algebra, `Held` state, and the mission graph
with reachability, spheres, solvability, and loud validation. Pure functions over plain data;
zero spatial concerns.

**Required reading**: [redesign 03 (all)](../redesign/03-mission-graph.md); the Rule/volatile
parts of [redesign 06](../redesign/06-puzzles-locks-and-recipes.md).

**Prerequisites**: M00 green.

**Porting references** (adapt, spec wins): legacy `core/src/logic/{rule,held}.ts`,
`core/src/graph/{region-graph,reachability,spheres,solvable}.ts`.

## Phase 1.1 — `core/src/logic/`

`rule.ts` — the exact tagged union from redesign 03 (“The Rule algebra”), plus builders and
queries:

```ts
export const ALWAYS: Rule;
export function have(cap: CapabilityId): Rule;
export function count(cap: CapabilityId, n: number): Rule;
export function flag(name: string, opts?: { volatile?: boolean }): Rule;
export function not(of: Rule): Rule;  export function and(...of: Rule[]): Rule;  export function or(...of: Rule[]): Rule;
export function evalRule(rule: Rule, held: Held): boolean;
export function ruleCaps(rule: Rule): CapabilityId[];          // every cap mentioned (deduped)
export function ruleFlags(rule: Rule): string[];
export function missingCaps(rule: Rule, held: Held): CapabilityId[]; // FULL unmet subset of a compound gate
export function usesVolatileFlag(rule: Rule): boolean;         // transitively references any volatile flag
```

`held.ts` — `Held` interface (`hasCap(cap): boolean`, `capCount(cap): number`,
`hasFlag(name): boolean`) + `CapSet` concrete implementation storing cap→count and a flag set,
with `add(cap)`, `addFlag(name)`, `clone()`, and `heldOf(caps, flags)` convenience.
`HeldData` = the plain serializable form + converters. Derived capabilities are resolved by the
capability layer (M04) when *building* a `Held` — `evalRule` stays ignorant of derivation.

Volatile semantics here: `evalRule` on a volatile flag checks the same flag store (the *caller*
decides whether volatile flags are populated — the solver never populates them; the sim clears
them on Space exit). `usesVolatileFlag` exists so M04's registry validation can reject
`required`-class conditions that touch one.

`logic/index.ts` barrel; export from `core/src/index.ts`.

## Phase 1.2 — `core/src/graph/`

`mission-graph.ts` — the exact shapes from redesign 03 (“The graph itself”): `Region` (with
`role: NodeRole`), `Edge`, `FlagDef` (with `setBy` provenance), `MissionGraph`, plus
`NodeRole`, `LocationId`, `RegionId` types.

`reachability.ts`:

```ts
export function reachableRegions(graph: MissionGraph, held: Held): Set<RegionId>;   // fixed-point BFS, directed
export function reachableLocations(graph: MissionGraph, held: Held): LocationId[];  // deterministic order: graph insertion order
```

`spheres.ts` — `computeSpheres(graph, startHeld, placement, items): SphereResult` per redesign 03:
iterate reach → collect newly reachable placed items → fold grants/flags (non-volatile only) →
repeat; returns `{ spheres: LocationId[][], heldPerSphere: HeldData[] }`.

`solvable.ts` — `isSolvable(graph, startHeld, items, placement): boolean` (every progression
capability collectible AND every non-`bonus` Location reachable when fully equipped, from
`startHeld`).

`validate.ts` — `validateGraph(graph, fullyEquippedHeld): void` throwing a `GenError` (define a
minimal `GenError` class in `core/src/errors.ts` now; M10 extends it) whose message lists the
stranded Region ids and, for each, the frontier edges whose rules were last unsatisfiable. Also
validate flag provenance here: every flag referenced by any edge rule must have a `FlagDef` with a
`setBy`; missing ⇒ `GenError` naming the flag.

`graph/index.ts` barrel; export from `core/src/index.ts`.

## Phase 1.3 — Tests

`logic/rule.test.ts`:
- truth tables for every connective incl. nesting; `count` boundary (`n−1` fails, `n` passes);
  `not` over `flag`.
- `missingCaps(and(have(a), have(b), flag(f)), heldWith(a))` returns exactly `[b]` (flags are not
  caps) — and an `or` with one satisfied branch returns `[]`.
- `usesVolatileFlag` true through arbitrary nesting, false otherwise.

`graph/graph.test.ts` (hand-built graphs, no RNG):
- linear chain gated mid-way: reachability grows exactly when the cap is added.
- one-way drop: forward reachable, reverse not; a one-way into a dead pocket makes
  `validateGraph` throw naming the pocket Region.
- counted key: gate `count(k, 3)`; spheres order the three copies before the gate.
- cross-Region flag: flag set by a Location in Region A gates an edge into Region C — reachable
  only after the setter is reachable.
- `computeSpheres` on a 3-sphere fixture returns the exact expected partition.
- `isSolvable` false when a progression item sits behind its own grant (hand-built broken
  placement); true for the fixed version.
- `validateGraph` diagnostic content: assert the message contains the stranded Region id **and**
  the blocking rule's caps (string containment is fine).

## Definition of Done

- [ ] All Phase 1.3 tests green; full root suite green; typecheck green.
- [ ] `missingCaps` never collapses a compound gate to one “primary” cap (explicit test).
- [ ] No RNG anywhere in `logic/` or `graph/` (they are deterministic *given inputs* — draws
      happen in later layers).
- [ ] `GenError` exists in `core/src/errors.ts` and `validateGraph` throws it (not a bare Error).
- [ ] STOP — hand off. Do not commit.

## Pitfalls

- Reachability must be a **fixed-point** loop (an edge that fails early can pass after another
  edge admits its `from` Region later in the same evaluation) — a single pass is wrong.
- Keep every returned collection in deterministic order (insertion order of the graph arrays) —
  descriptor serialization later depends on it.
