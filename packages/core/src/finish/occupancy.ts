/**
 * Occupancy / collision grid — a world-grid-aligned solid/open mask sampled from
 * the area field (1 = solid, 0 = open). Cheap, deterministic, serializable; the
 * host reuses it for player/camera collision + navigation. `collideSphere` does
 * swept-sphere push-out + slide. OOB = solid (the world edge is a wall).
 */

import type { Vec3 } from "../math/index.js";
import type { Sdf } from "../volume/sdf.js";
import type { Coord } from "../volume/field.js";

export interface OccupancyGrid {
  origin: Vec3;
  res: number;
  dims: Coord;
  solid: Uint8Array; // index (k*dy+j)*dx+i
}

/** Serializable form (solid as a plain number[]). */
export interface OccupancyData {
  origin: [number, number, number];
  res: number;
  dims: [number, number, number];
  solid: number[];
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
  return [Math.floor((p[0] - grid.origin[0]) / grid.res), Math.floor((p[1] - grid.origin[1]) / grid.res), Math.floor((p[2] - grid.origin[2]) / grid.res)];
}

/** Solid (or out-of-bounds → solid) at a world point. */
export function isSolidAt(grid: OccupancyGrid, p: Vec3): boolean {
  const [i, j, k] = cellOf(grid, p);
  const [dx, dy, dz] = grid.dims;
  if (i < 0 || j < 0 || k < 0 || i >= dx || j >= dy || k >= dz) return true;
  return grid.solid[gIdx(grid.dims, i, j, k)] === 1;
}

/** Walkable = open cell with solid support directly below. */
export function isWalkable(grid: OccupancyGrid, coord: Coord): boolean {
  const [i, j, k] = coord;
  const [dx, dy, dz] = grid.dims;
  if (i < 0 || j < 0 || k < 0 || i >= dx || j >= dy || k >= dz) return false;
  if (grid.solid[gIdx(grid.dims, i, j, k)] === 1) return false;
  if (k === 0) return true; // floor of the grid supports
  return grid.solid[gIdx(grid.dims, i, j, k - 1)] === 1;
}

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
          const minx = origin[0] + i * res;
          const miny = origin[1] + j * res;
          const minz = origin[2] + k * res;
          const qx = Math.max(minx, Math.min(p[0], minx + res));
          const qy = Math.max(miny, Math.min(p[1], miny + res));
          const qz = Math.max(minz, Math.min(p[2], minz + res));
          const nx = p[0] - qx;
          const ny = p[1] - qy;
          const nz = p[2] - qz;
          const d = Math.hypot(nx, ny, nz);
          if (d >= r) continue;
          if (d < 1e-6) {
            const cx = minx + res / 2;
            const cy = miny + res / 2;
            const cz = minz + res / 2;
            const dxp = res / 2 - Math.abs(p[0] - cx);
            const dyp = res / 2 - Math.abs(p[1] - cy);
            const dzp = res / 2 - Math.abs(p[2] - cz);
            if (dxp <= dyp && dxp <= dzp) p[0] += (p[0] < cx ? -1 : 1) * (dxp + r);
            else if (dyp <= dzp) p[1] += (p[1] < cy ? -1 : 1) * (dyp + r);
            else p[2] += (p[2] < cz ? -1 : 1) * (dzp + r);
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

export function toOccupancyData(grid: OccupancyGrid): OccupancyData {
  return { origin: [grid.origin[0], grid.origin[1], grid.origin[2]], res: grid.res, dims: [grid.dims[0], grid.dims[1], grid.dims[2]], solid: Array.from(grid.solid) };
}
