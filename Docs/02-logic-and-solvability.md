# 02 · Logic & solvability

CycleVania adapts the Archipelago randomizer's formal model: **Regions** (nodes with gated entrances),
**Locations** (placeable slots), **Items** (progression/useful/filler), **Spheres** (the solvability
ladder), and **assumed fill** (place items so solvability is *constructed*, not checked-and-retried).

## Rules (`logic/`)

A `Rule` gates an edge. Beyond the classic connectives it supports counted keys and event flags:

```ts
type Rule =
  | { k: "always" }
  | { k: "have"; cap }                 // hold a capability
  | { k: "count"; cap; n }             // hold ≥ n of a capability (multi-key locks)
  | { k: "flag"; name }                // an event/switch flag is set
  | { k: "not"; of } | { k: "and"; of } | { k: "or"; of };
```

Rules evaluate against a `Held` (`has` / `count` / `flag`). `CapSet` is the concrete implementation;
`heldOf(caps, flags)` builds one. `missingCaps(rule, held)` returns the *unmet* subset of a compound gate
— use it for "remembered locks" UI so an A∧B door never shows just one "primary" cap.

## The graph (`graph/`)

`reachableRegions(graph, held)` is a fixed-point BFS following an edge only when its rule passes (directed,
so one-way drops are handled). `computeSpheres` collects items sphere by sphere; `isSolvable` is the
zero-softlock guarantee (every progression cap collectible AND every non-bonus location reachable).
`validateGraph(graph)` is a construction-time precondition: every region must be reachable when fully
equipped — if not, it reports the stranded regions (fail loudly).

## Assumed fill (`fill/`)

To place an item, assume the party holds **every other** unplaced item, find the reachable empty
locations, and drop it in one. Inductively an item is only ever gated behind items placed *after* it — a
valid sphere ordering. Counted keys work for free: placing the Nth of a counted item sees only N−1
assumed, so a `count(cap, N)` gate can't hide the last copy behind itself.

The invariants that make ANY well-formed template softlock-impossible:
1. hub nodes over-provision `≥ items + 1` always-reachable bootstrap slots;
2. `validateGraph` passes (all regions reachable fully-equipped);
3. `assumedFill` → `isSolvable` closes the loop.

The examples soak runs 1000 seeds with **zero solver failures**.
