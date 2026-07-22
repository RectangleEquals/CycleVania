/**
 * Field composition — an Area's open-space SDF is the smooth union of every room
 * hull + every connector tube (negative = open/walkable). The geometry pass
 * contours this field's 0-isosurface into the actual wall/floor/ceiling geometry
 * and reads its sign for the occupancy/collision grid.
 */

import type { Sdf } from "./sdf.js";
import { smoothUnion, union } from "./sdf.js";

export interface AreaField {
  sdf: Sdf;
  /** true where the point is inside open/walkable space. */
  isOpen: (p: import("../math/vec.js").Vec3) => boolean;
}

export function composeAreaField(hulls: readonly Sdf[], connectors: readonly Sdf[], smoothK = 1.5): AreaField {
  const all = [...hulls, ...connectors];
  // smooth-union the hulls (organic blends at overlaps); connectors join hard enough to stay tube-like.
  const sdf = all.length === 0 ? () => 1 : hulls.length > 0 ? union([smoothUnion(smoothK, hulls), ...connectors]) : union(all);
  return { sdf, isOpen: (p) => sdf(p) < 0 };
}
