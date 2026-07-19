/**
 * Nested descriptor assembler — bundles composed Reaches into a single top-level
 * `WorldDescriptor` (the shape a game consumes for a whole run).
 */

import { boxUnionAll } from "../math/geom.js";
import type { ReachDescriptor, WorldDescriptor } from "./descriptor.js";

export function assembleWorld(reaches: readonly ReachDescriptor[]): WorldDescriptor {
  const bounds = reaches.length > 0 ? boxUnionAll(reaches.map((r) => r.bounds)) : { min: [0, 0, 0] as const, max: [1, 1, 1] as const };
  return {
    reaches: [...reaches],
    bounds,
    meta: { reachCount: reaches.length, areaCount: reaches.reduce((n, r) => n + r.areas.length, 0) },
  };
}
