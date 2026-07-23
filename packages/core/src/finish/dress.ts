/**
 * Naturalization dressing — deterministic noise-driven anchors the game instantiates
 * as props: water pools in low open cells, stalactites where open meets ceiling,
 * foliage/rubble on floors. Data only (position + kind + up), no meshes.
 */

import type { Vec3 } from "../math/vec.js";
import { fbm3 } from "../math/noise.js";
import type { OccupancyGrid } from "./occupancy.js";

export interface DressingAnchor {
  pos: Vec3;
  kind: "water" | "stalactite" | "foliage" | "rubble";
  up: Vec3;
}

const gIdx = (dx: number, dy: number, i: number, j: number, k: number): number => (k * dy + j) * dx + i;

export function dressArea(grid: OccupancyGrid, seed: number, density = 0.35): DressingAnchor[] {
  const { origin, res, dims, solid } = grid;
  const [dx, dy, dz] = dims;
  const out: DressingAnchor[] = [];
  const at = (i: number, j: number, k: number): boolean => (i < 0 || j < 0 || k < 0 || i >= dx || j >= dy || k >= dz ? true : solid[gIdx(dx, dy, i, j, k)] === 1);
  // lowest open cell per column → water level
  const waterK = new Int32Array(dx * dy).fill(-1);
  for (let j = 0; j < dy; j++)
    for (let i = 0; i < dx; i++)
      for (let k = 0; k < dz; k++)
        if (!at(i, j, k)) {
          waterK[j * dx + i] = k;
          break;
        }

  for (let k = 0; k < dz; k++)
    for (let j = 0; j < dy; j++)
      for (let i = 0; i < dx; i++) {
        if (at(i, j, k)) continue; // only open cells host props
        const c: Vec3 = [origin[0] + (i + 0.5) * res, origin[1] + (j + 0.5) * res, origin[2] + (k + 0.5) * res];
        const n = fbm3(c[0] * 0.15, c[1] * 0.15, c[2] * 0.15, seed, 3);
        if ((n + 1) / 2 > 1 - density * 0.5) {
          if (at(i, j, k + 1)) {
            out.push({ pos: [c[0], c[1], origin[2] + (k + 1) * res], kind: "stalactite", up: [0, 0, -1] });
            continue;
          }
          if (at(i, j, k - 1)) {
            const floorZ = origin[2] + k * res;
            const kind = k === waterK[j * dx + i] && n < 0 ? "water" : n > 0.3 ? "rubble" : "foliage";
            out.push({ pos: [c[0], c[1], floorZ], kind, up: [0, 0, 1] });
          }
        }
      }
  return out;
}
