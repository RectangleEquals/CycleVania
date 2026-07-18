/**
 * Assumed fill — the placement algorithm that makes softlocks impossible by
 * CONSTRUCTION rather than by check-and-retry. To place an item, assume the party
 * holds every OTHER unplaced progression item, find the reachable empty locations,
 * and drop the item in one. Inductively this yields a valid sphere ordering: an
 * item is only ever gated behind items placed after it. An independent
 * reachability check (`isSolvable`) confirms each run.
 *
 * Counted keys generalize for free: N copies of an item share one capability, so
 * "assume all others held" gives count N-1 while placing the Nth — a count(cap,N)
 * gate can never hide the last copy behind itself.
 */

import { CapSet, type Capability } from "../logic/index.js";
import { shuffle, type Rng } from "../math/rng.js";
import { isSolvable } from "../graph/solvable.js";
import { reachableLocations } from "../graph/reachability.js";
import type { LocationId, Placement, ProgressionItem, RegionGraph } from "../graph/region-graph.js";

export interface AssumedFillOptions {
  /** Fresh-order retries before giving up (fill-error recovery). */
  retries?: number;
  /** Locations that may remain unreachable this pass (future-sphere / loot). */
  bonus?: ReadonlySet<LocationId>;
}

/**
 * Place all `items` so the result is solvable from `startCaps`. Returns null only
 * if no order works (malformed graph: too few reachable locations) — callers that
 * validated the graph first treat null as a bug and throw.
 */
export function assumedFill(
  g: RegionGraph,
  items: readonly ProgressionItem[],
  startCaps: Iterable<Capability>,
  rng: Rng,
  opts: AssumedFillOptions = {},
): Placement | null {
  const retries = opts.retries ?? 30;
  const bonus = opts.bonus;
  const startList = [...startCaps];
  const itemsById = new Map(items.map((i) => [i.id, i] as const));

  for (let attempt = 0; attempt < retries; attempt++) {
    const order = shuffle(items.slice(), rng);
    const placement: Placement = new Map();

    // assume ALL items held, plus starting caps
    const assumed = new CapSet();
    for (const it of order) assumed.add(it.grants, 1);
    for (const cap of startList) assumed.add(cap, 1);

    let ok = true;
    for (const item of order) {
      assumed.add(item.grants, -1); // don't assume the one we're placing
      const empty: LocationId[] = [];
      for (const l of reachableLocations(g, assumed)) if (!placement.has(l)) empty.push(l);
      if (empty.length === 0) {
        ok = false;
        break;
      }
      placement.set(empty[Math.floor(rng.next() * empty.length)] as LocationId, item.id);
    }

    if (ok && isSolvable(g, placement, itemsById, startList, bonus)) return placement;
  }
  return null;
}
