/**
 * Occupancy / collision grid — a world-grid-aligned solid/open mask sampled from the
 * area field (1 = solid rock, 0 = open air). Cheap, deterministic, serializable; the
 * game reuses it for player/camera collision. `collideSphere` does swept-sphere
 * push-out + slide against the solid cells (no clipping through faceted walls).
 */

import type { Vec3 } from "../math/vec.js";
import type { Coord } from "../spatial/grid.js";
import type { Sdf } from "../volume/sdf.js";

export interface OccupancyGrid {
  origin: Vec3;
  res: number;
  dims: Coord; // cell counts in x,y,z
  solid: Uint8Array; // index (k*dy+j)*dx+i, 1 = solid
}

const gIdx = (dims: Coord, i: number, j: number, k: number): number => (k * dims[1] + j) * dims[0] + i;

export function occupancyGrid(field: Sdf, origin: Vec3, dims: Coord, res: number): OccupancyGrid {
  const [dx, dy, dz] = dims;
  const solid = new Uint8Array(dx * dy * dz);
  for (let k = 0; k < dz; k++)
    for (let j = 0; j < dy; j++)
      for (let i = 0; i < dx; i++) {
        const c: Vec3 = [origin[0] + (i + 0.5) * res, origin[1] + (j + 0.5) * res, origin[2] + (k + 0.5) * res];
        solid[gIdx(dims, i, j, k)] = field(c) > 0 ? 1 : 0;
      }
  return { origin, res, dims, solid };
}

export function cellOf(grid: OccupancyGrid, p: Vec3): Coord {
  return [
    Math.floor((p[0] - grid.origin[0]) / grid.res),
    Math.floor((p[1] - grid.origin[1]) / grid.res),
    Math.floor((p[2] - grid.origin[2]) / grid.res),
  ];
}

/** Solid (or out-of-bounds → solid, so you can't leave the area) at world point. */
export function isSolidAt(grid: OccupancyGrid, p: Vec3): boolean {
  const [i, j, k] = cellOf(grid, p);
  const [dx, dy, dz] = grid.dims;
  if (i < 0 || j < 0 || k < 0 || i >= dx || j >= dy || k >= dz) return true;
  return grid.solid[gIdx(grid.dims, i, j, k)] === 1;
}

/** Swept-sphere push-out: resolve `pos` (radius r) out of every overlapping solid cell. */
export function collideSphere(grid: OccupancyGrid, pos: Vec3, r: number): Vec3 {
  const { origin, res, dims, solid } = grid;
  const [dx, dy, dz] = dims;
  const p: [number, number, number] = [pos[0], pos[1], pos[2]];
  for (let iter = 0; iter < 4; iter++) {
    let moved = false;
    const lo = [Math.floor((p[0] - r - origin[0]) / res), Math.floor((p[1] - r - origin[1]) / res), Math.floor((p[2] - r - origin[2]) / res)];
    const hi = [Math.floor((p[0] + r - origin[0]) / res), Math.floor((p[1] + r - origin[1]) / res), Math.floor((p[2] + r - origin[2]) / res)];
    for (let k = lo[2] as number; k <= (hi[2] as number); k++)
      for (let j = lo[1] as number; j <= (hi[1] as number); j++)
        for (let i = lo[0] as number; i <= (hi[0] as number); i++) {
          const oob = i < 0 || j < 0 || k < 0 || i >= dx || j >= dy || k >= dz;
          if (!oob && solid[gIdx(dims, i, j, k)] !== 1) continue;
          // nearest point on this cell's AABB to p
          const minx = origin[0] + i * res;
          const miny = origin[1] + j * res;
          const minz = origin[2] + k * res;
          const qx = Math.max(minx, Math.min(p[0], minx + res));
          const qy = Math.max(miny, Math.min(p[1], miny + res));
          const qz = Math.max(minz, Math.min(p[2], minz + res));
          let nx = p[0] - qx;
          let ny = p[1] - qy;
          let nz = p[2] - qz;
          let d = Math.hypot(nx, ny, nz);
          if (d >= r) continue;
          if (d < 1e-6) {
            // centre inside solid cell: push along axis of least penetration to cell face
            const cx = minx + res / 2;
            const cy = miny + res / 2;
            const cz = minz + res / 2;
            const dxp = res / 2 - Math.abs(p[0] - cx);
            const dyp = res / 2 - Math.abs(p[1] - cy);
            const dzp = res / 2 - Math.abs(p[2] - cz);
            if (dxp <= dyp && dxp <= dzp) { p[0] += (p[0] < cx ? -1 : 1) * (dxp + r); }
            else if (dyp <= dzp) { p[1] += (p[1] < cy ? -1 : 1) * (dyp + r); }
            else { p[2] += (p[2] < cz ? -1 : 1) * (dzp + r); }
            moved = true;
            continue;
          }
          const push = (r - d) / d;
          p[0] += nx * push;
          p[1] += ny * push;
          p[2] += nz * push;
          moved = true;
        }
    if (!moved) break;
  }
  return p;
}
