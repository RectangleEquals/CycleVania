/** Line-of-sight through the composed field — the basis for vista placement. */

import { add, sub, scale, normalize, length, type Vec3 } from "../math/index.js";
import type { Sdf } from "../volume/sdf.js";

/** True if the segment a→b stays in (mostly) open space — no solid rock between. */
export function hasLineOfSight(field: Sdf, a: Vec3, b: Vec3, step = 2): boolean {
  const d = sub(b, a);
  const len = length(d);
  if (len < step) return true;
  const dir = normalize(d);
  for (let t = step; t < len - step; t += step) {
    if (field(add(a, scale(dir, t))) > step) return false;
  }
  return true;
}
