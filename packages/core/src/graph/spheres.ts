/**
 * Spheres — the solvability ladder. Sphere 0 = reachable from start with
 * starting capabilities; sphere N+1 = reachable once items in spheres ≤ N are
 * collected. Playing forward from `startCaps`, collect every reachable item,
 * expanding the held state until a fixed point.
 */

import { CapSet, heldOf, type Capability } from "../logic/index.js";
import { reachableLocations } from "./reachability.js";
import type { LocationId, Placement, ProgressionItem, RegionGraph } from "./region-graph.js";

export interface SphereResult {
  /** location ids first reachable at each sphere, in order. */
  spheres: LocationId[][];
  /** total capabilities collectible playing forward. */
  held: CapSet;
  /** every NON-bonus location is reachable (no stranded required slots). */
  reachedAll: boolean;
}

/** Collect items sphere by sphere from a placement, to a fixed point. */
export function computeSpheres(
  g: RegionGraph,
  placement: ReadonlyMap<LocationId, string>,
  itemsById: ReadonlyMap<string, ProgressionItem>,
  startCaps: Iterable<Capability> = [],
  bonus?: ReadonlySet<LocationId>,
): SphereResult {
  const held = heldOf(startCaps);
  const collected = new Set<LocationId>();
  const spheres: LocationId[][] = [];
  for (;;) {
    const newly: LocationId[] = [];
    for (const l of reachableLocations(g, held)) if (!collected.has(l)) newly.push(l);
    if (newly.length === 0) break;
    spheres.push(newly);
    for (const l of newly) {
      collected.add(l);
      const id = placement.get(l);
      const item = id ? itemsById.get(id) : undefined;
      if (item) held.add(item.grants, 1);
    }
  }
  let reachedAll = true;
  for (const l of g.locations.keys()) {
    if ((!bonus || !bonus.has(l)) && !collected.has(l)) {
      reachedAll = false;
      break;
    }
  }
  return { spheres, held, reachedAll };
}
