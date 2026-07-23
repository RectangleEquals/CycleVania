/**
 * Reachability — fixed-point BFS over region edges, following an edge only when
 * its rule is satisfied. Directed edges handle one-way drops naturally (no
 * reverse edge is followed). Every returned collection is in deterministic
 * (graph insertion) order.
 */

import { evalRule, type Held } from "../logic/index.js";
import type { LocationId, MissionGraph, RegionId } from "./mission-graph.js";

/** Regions reachable from `start` given held state (fixed-point). */
export function reachableRegions(g: MissionGraph, held: Held): Set<RegionId> {
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

/**
 * Locations reachable given held state: their region is reachable AND their own
 * gate (if any) passes. Returned in `graph.locations` array order.
 */
export function reachableLocations(g: MissionGraph, held: Held): LocationId[] {
  const rr = reachableRegions(g, held);
  const out: LocationId[] = [];
  for (const loc of g.locations) {
    if (rr.has(loc.region) && (loc.gate === undefined || evalRule(loc.gate, held))) out.push(loc.id);
  }
  return out;
}
