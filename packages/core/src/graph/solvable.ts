/**
 * Solvability checks — the independent reachability regression the fill is
 * verified against. A world is solvable when, playing forward from starting
 * capabilities: every progression item is collectible AND every non-bonus
 * location is reachable (no stranded required slots).
 */

import type { Capability, Held } from "../logic/index.js";
import { computeSpheres } from "./spheres.js";
import { reachableRegions } from "./reachability.js";
import type { LocationId, ProgressionItem, RegionGraph, RegionId } from "./region-graph.js";

/** The zero-softlock guarantee. */
export function isSolvable(
  g: RegionGraph,
  placement: ReadonlyMap<LocationId, string>,
  itemsById: ReadonlyMap<string, ProgressionItem>,
  startCaps: Iterable<Capability> = [],
  bonus?: ReadonlySet<LocationId>,
): boolean {
  const { held, reachedAll } = computeSpheres(g, placement, itemsById, startCaps, bonus);
  for (const it of itemsById.values()) if (!held.has(it.grants)) return false;
  return reachedAll;
}

/** Does the graph contain a directed cycle (ignoring access rules)? */
export function hasCycle(g: RegionGraph): boolean {
  const adj = new Map<RegionId, RegionId[]>();
  for (const e of g.edges) {
    let list = adj.get(e.from);
    if (!list) adj.set(e.from, (list = []));
    list.push(e.to);
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
  for (const r of g.regions) if ((color.get(r) ?? WHITE) === WHITE && visit(r)) return true;
  return false;
}

/** A `Held` that satisfies everything — used to validate full-equipment reachability. */
const OMNISCIENT: Held = {
  has: () => true,
  count: () => Number.MAX_SAFE_INTEGER,
  flag: () => true,
};

export interface GraphValidation {
  ok: boolean;
  /** regions unreachable even when every capability/flag is held. */
  stranded: RegionId[];
}

/**
 * Construction-time precondition for softlock-impossible fill: every region must
 * be reachable when the party holds everything. If not, the template/embedding is
 * malformed (a region gated behind an impossible or dangling edge) — fail loudly
 * with the exact stranded set rather than an opaque "fill failed".
 */
export function validateGraph(g: RegionGraph): GraphValidation {
  const reached = reachableRegions(g, OMNISCIENT);
  const stranded = [...g.regions].filter((r) => !reached.has(r));
  return { ok: stranded.length === 0, stranded };
}
