/**
 * Reachability — fixed-point BFS over region edges, following an edge only when
 * its access rule is satisfied by the held state. Directed edges mean a one-way
 * drop is naturally handled (no reverse edge is followed).
 */

import { evalRule, type Held } from "../logic/index.js";
import type { LocationId, RegionGraph, RegionId } from "./region-graph.js";

/** Regions reachable from `start` given held state (fixed-point BFS). */
export function reachableRegions(g: RegionGraph, held: Held): Set<RegionId> {
  const reached = new Set<RegionId>([g.start]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const e of g.edges) {
      if (reached.has(e.from) && !reached.has(e.to) && evalRule(e.rule, held)) {
        reached.add(e.to);
        changed = true;
      }
    }
  }
  return reached;
}

/** Locations whose region is reachable given held state. */
export function reachableLocations(g: RegionGraph, held: Held): Set<LocationId> {
  const rr = reachableRegions(g, held);
  const out = new Set<LocationId>();
  for (const [loc, reg] of g.locations) if (rr.has(reg)) out.add(loc);
  return out;
}
