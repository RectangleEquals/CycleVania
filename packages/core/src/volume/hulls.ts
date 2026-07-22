/**
 * Hull archetypes — parametric SDF recipes for a room's OPEN volume (negative
 * inside). Built-in procedural archetypes (the game supplies which archetype +
 * size + biome per room kind); authored landmark hulls plug in via `custom`.
 * Selecting a bigger/noisier/`outdoor-open` hull is how areas get variety.
 */

import type { Vec3 } from "../math/vec.js";
import { type Sdf, box, capsule, displace, ellipsoid, roundBox } from "./sdf.js";

export interface HullParams {
  center: Vec3;
  size: Vec3; // full extent (x,y,z)
  seed: number;
  noise?: number; // 0..1 organic warp amount
}

export type HullArchetype = (p: HullParams) => Sdf;

const minDim = (s: Vec3): number => Math.min(s[0], s[1], s[2]);
const maxDim = (s: Vec3): number => Math.max(s[0], s[1], s[2]);

export const HULL_ARCHETYPES: Record<string, HullArchetype> = {
  /** rounded hall — mostly rectilinear, softened edges. */
  hall: ({ center, size, seed, noise = 0.3 }) =>
    displace(roundBox(center, [size[0] / 2, size[1] / 2, size[2] / 2], minDim(size) * 0.14), noise * minDim(size) * 0.05, 1.4 / maxDim(size), seed),

  /** rotunda — smooth ellipsoidal chamber. */
  rotunda: ({ center, size, seed, noise = 0.4 }) =>
    displace(ellipsoid(center, [size[0] / 2, size[1] / 2, size[2] / 2]), noise * minDim(size) * 0.08, 1.6 / maxDim(size), seed),

  /** cavern — heavily noise-warped organic blob. */
  cavern: ({ center, size, seed, noise = 1 }) =>
    displace(ellipsoid(center, [size[0] / 2, size[1] / 2, size[2] / 2]), noise * minDim(size) * 0.22, 1.1 / maxDim(size), seed),

  /** shaft — tall vertical tube (verticality). */
  shaft: ({ center, size, seed, noise = 0.5 }) =>
    displace(
      capsule([center[0], center[1], center[2] - size[2] / 2], [center[0], center[1], center[2] + size[2] / 2], Math.min(size[0], size[1]) / 2),
      noise * Math.min(size[0], size[1]) * 0.08,
      1.5 / maxDim(size),
      seed,
    ),

  /** outdoor-open — a wide volume open to the sky (its "ceiling" sits far above the meshing bounds). */
  "outdoor-open": ({ center, size, seed, noise = 1 }) =>
    displace(
      box([center[0], center[1], center[2] + size[2]], [size[0] / 2, size[1] / 2, size[2] * 1.5]),
      noise * minDim(size) * 0.16,
      1 / maxDim(size),
      seed,
    ),
};

export function hull(archetype: string, params: HullParams): Sdf {
  const fn = HULL_ARCHETYPES[archetype] ?? HULL_ARCHETYPES["hall"];
  return (fn as HullArchetype)(params);
}
