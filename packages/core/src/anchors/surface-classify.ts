/**
 * Surface classification from an OPEN-facing normal (the −gradient of the hull
 * SDF, pointing into walkable space). Shared verbatim by the finish pass so
 * anchors and geometry always agree. Cones: 0.6 / 0.25 in normal.z.
 */

import type { Vec3 } from "../math/index.js";
import type { SurfaceKind } from "../types.js";

export function classifySurface(openNormal: Vec3): SurfaceKind {
  const nz = openNormal[2];
  if (nz > 0.6) return "floor";
  if (nz < -0.6) return "ceiling";
  if (Math.abs(nz) <= 0.25) return "wall";
  if (nz > 0.25) return "slope";
  return "overhang";
}
