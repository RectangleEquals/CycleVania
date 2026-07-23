/**
 * Field composition — an Area's open-space SDF is the smooth union of every room
 * hull + every connector tube (negative = open/walkable). Carries the
 * voxelization extents (origin/dims/res) and the shared `forEachCell` iterator
 * (x→y→z order) that BOTH the L4 mesher and the L3 anchor scatter must use so
 * their cell enumeration agrees.
 */

import type { Vec3, WorldBox } from "../math/index.js";
import { boxUnionAll } from "../math/index.js";
import type { Sdf } from "./sdf.js";
import { smoothUnion, union } from "./sdf.js";

export type Coord = [number, number, number];

export interface FieldExtents {
  origin: Vec3; // world position of cell (0,0,0)'s min corner
  dims: Coord; // number of cells along x,y,z
  res: number; // world units per cell
}

export interface AreaField {
  sdf: Sdf;
  isOpen: (p: Vec3) => boolean;
  extents: FieldExtents;
}

/** Voxelization extents covering `boxes` + margin, at `res`, clamped so no axis exceeds `maxDim` cells (res coarsens to fit). */
export function fieldExtentsFrom(boxes: readonly WorldBox[], res: number, margin: number, maxDim: number): FieldExtents {
  if (boxes.length === 0) return { origin: [0, 0, 0], dims: [1, 1, 1], res };
  const b = boxUnionAll(boxes);
  const min: Vec3 = [b.min[0] - margin, b.min[1] - margin, b.min[2] - margin];
  const span: Vec3 = [b.max[0] - b.min[0] + 2 * margin, b.max[1] - b.min[1] + 2 * margin, b.max[2] - b.min[2] + 2 * margin];
  let r = res;
  const dimsAt = (rr: number): Coord => [Math.max(1, Math.ceil(span[0] / rr)), Math.max(1, Math.ceil(span[1] / rr)), Math.max(1, Math.ceil(span[2] / rr))];
  let dims = dimsAt(r);
  while (Math.max(dims[0], dims[1], dims[2]) > maxDim) {
    r *= 1.25;
    dims = dimsAt(r);
  }
  return { origin: min, dims, res: r };
}

/** Iterate cells in fixed x→y→z order, passing cell index + its world-space center. */
export function forEachCell(ext: FieldExtents, fn: (cx: number, cy: number, cz: number, center: Vec3) => void): void {
  const { origin, dims, res } = ext;
  for (let cx = 0; cx < dims[0]; cx++) {
    for (let cy = 0; cy < dims[1]; cy++) {
      for (let cz = 0; cz < dims[2]; cz++) {
        fn(cx, cy, cz, [origin[0] + (cx + 0.5) * res, origin[1] + (cy + 0.5) * res, origin[2] + (cz + 0.5) * res]);
      }
    }
  }
}

export function composeAreaField(hulls: readonly Sdf[], connectors: readonly Sdf[], extents: FieldExtents, smoothK = 1.5): AreaField {
  const all = [...hulls, ...connectors];
  const sdf: Sdf = all.length === 0 ? () => 1 : hulls.length > 0 ? union([smoothUnion(smoothK, hulls), ...connectors]) : union(all);
  return { sdf, isOpen: (p) => sdf(p) < 0, extents };
}
