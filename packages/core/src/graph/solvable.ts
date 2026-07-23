/**
 * Solvability — the zero-softlock guarantee and structural helpers. A Reach is
 * solvable when, playing forward from `startHeld`: every progression capability
 * its items introduce is collectible AND every non-bonus Location is reachable.
 */

import type { CapSet } from "../logic/index.js";
import { computeSpheres } from "./spheres.js";
import type { Item, MissionGraph, Placement, RegionId } from "./mission-graph.js";

export function isSolvable(
  g: MissionGraph,
  startHeld: CapSet,
  items: readonly Item[],
  placement: Placement,
): boolean {
  const { finalHeld, reachedAll } = computeSpheres(g, startHeld, placement, items);
  for (const it of items) {
    if (it.class !== "progression" || !it.grants) continue;
    for (const cap of it.grants) if (!finalHeld.hasCap(cap)) return false;
  }
  return reachedAll;
}

/** Does the graph contain a directed cycle (ignoring access rules)? */
export function hasCycle(g: MissionGraph): boolean {
  const adj = new Map<RegionId, RegionId[]>();
  for (const e of g.edges) {
    const list = adj.get(e.from);
    if (list) list.push(e.to);
    else adj.set(e.from, [e.to]);
  }
  const WHITE = 0;
  const GRAY = 1;
  const BLACK = 2;
  const color = new Map<RegionId, number>();
  const visit = (u: RegionId): boolean => {
    color.set(u, GRAY);
    for (const v of adj.get(u) ?? []) {
      const c = color.get(v) ?? WHITE;
      if (c === GRAY) return true;
      if (c === WHITE && visit(v)) return true;
    }
    color.set(u, BLACK);
    return false;
  };
  for (const r of g.regions) if ((color.get(r.id) ?? WHITE) === WHITE && visit(r.id)) return true;
  return false;
}
