# 03 · Reach templates

A `ReachTemplate` is the macro-structure as **data** — roles, slot policy, gating, branches, loops — but
never concrete capabilities (those come from the item catalog). The default demo template reproduces the
classic "hub + 5 segments + capstone + terminal + vault branches + back-edges" cadence; a game can declare
any shape (mini 3-area reaches, 8-area sprawls, hub-and-spoke, one giant Reach à la Metroid Prime).

## Roles

`hub` (start/save; over-provisions bootstrap slots) · `segment` (critical-path progress) · `gate` (its
entrance is the primary lock) · `vault` (optional branch/loot) · `capstone` (boss/set-piece) · `terminal`
(exit / next-hub handoff).

## Shape

```ts
interface ReachTemplate {
  criticalPath: string[];                 // ordered node ids
  nodes: Record<string, TemplateNode>;    // { id, role, slots:{min,max,class?}, bootstrap? }
  branches: BranchSpec[];                 // vaults hung off segments, with entrance + optional back-edge
  gating: { lockFraction; compoundChance; keepEntryOpen; keepExitOpen };
  loops: { guaranteeAtLeastOne };
}
```

`generateReach({ seed, template, items, startCaps })` interprets it into a `GeneratedReach`
(`{ graph, items, placement, startCaps, meta }`). It gates a `lockFraction` of the internal critical-path
edges (a `gate`-roled node always locks its entrance), hangs vault branches with `single`/`compound`/
`optional-open` entrances, closes at least one loop when asked, then runs `validateGraph` + `assumedFill`
and **throws** on failure — a malformed template is a bug, not runtime input.

## Guarantee

Because the three solvability invariants (see [02](./02-logic-and-solvability.md)) are enforced regardless
of the template's shape, every well-formed template yields a softlock-free, backtracking-capable Reach.
