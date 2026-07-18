/**
 * Item classes + non-progression fill. Progression items are placed by
 * `assumedFill` (solvability-critical); everything else fills the remaining
 * locations by a simple risk/biome-agnostic policy the game can weight.
 */

import type { Rng } from "../math/rng.js";
import type { LocationId, Placement, RegionGraph } from "../graph/region-graph.js";

export type ItemClass = "progression" | "useful" | "filler" | "bonus";

/**
 * Fill every location left empty by the progression pass with items drawn from
 * `pool` (useful/filler ids). Deterministic given `rng`. Locations already in
 * `placement` are left untouched.
 */
export function fillRemaining(
  g: RegionGraph,
  placement: Placement,
  pool: readonly string[],
  rng: Rng,
): Placement {
  if (pool.length === 0) return placement;
  for (const loc of g.locations.keys()) {
    if (!placement.has(loc)) placement.set(loc as LocationId, pool[rng.int(0, pool.length - 1)] as string);
  }
  return placement;
}
