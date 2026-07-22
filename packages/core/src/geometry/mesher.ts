/**
 * Dual contouring — extract the field's 0-isosurface (the room walls/floor/ceiling)
 * into a mesh. One QEF-placed vertex per surface-straddling cell (sharp features),
 * quads across sign-changing grid edges. Surface normals face the OPEN interior and
 * are 5°-quantized (the PS2 facet look). Deterministic (our QEF + math, no host trig).
 */

import type { Vec3 } from "../math/vec.js";
import type { Coord } from "../spatial/grid.js";
import { clamp } from "../math/curve.js";
import { solveQEF } from "../math/qef.js";
import { sdfNormal, type Sdf } from "../volume/sdf.js";
import { quantizeNormal } from "./fidelity.js";

export interface Mesh {
  positions: number[];
  normals: number[];
  indices: number[];
}

const CORNERS: ReadonlyArray<readonly [number, number, number]> = [
  [0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0], [0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1],
];
const EDGES: ReadonlyArray<readonly [number, number]> = [
  [0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 7], [7, 4], [0, 4], [1, 5], [2, 6], [3, 7],
];

export function dualContour(field: Sdf, origin: Vec3, dims: Coord, res: number, snap = true): Mesh {
  const dx = dims[0];
  const dy = dims[1];
  const dz = dims[2];
  const nx = dx + 1;
  const ny = dy + 1;
  const nz = dz + 1;
  const val = new Float64Array(nx * ny * nz);
  const gi = (i: number, j: number, k: number): number => (k * ny + j) * nx + i;
  for (let k = 0; k < nz; k++)
    for (let j = 0; j < ny; j++)
      for (let i = 0; i < nx; i++) val[gi(i, j, k)] = field([origin[0] + i * res, origin[1] + j * res, origin[2] + k * res]);

  const positions: number[] = [];
  const normals: number[] = [];
  const vertIdx = new Int32Array(dx * dy * dz).fill(-1);
  const ci = (i: number, j: number, k: number): number => (k * dy + j) * dx + i;

  for (let k = 0; k < dz; k++)
    for (let j = 0; j < dy; j++)
      for (let i = 0; i < dx; i++) {
        const cv: number[] = [];
        for (let c = 0; c < 8; c++) {
          const o = CORNERS[c] as readonly [number, number, number];
          cv.push(val[gi(i + o[0], j + o[1], k + o[2])] as number);
        }
        const pts: Vec3[] = [];
        const nrms: Vec3[] = [];
        for (const [a, b] of EDGES) {
          const va = cv[a] as number;
          const vb = cv[b] as number;
          if (va <= 0 === (vb <= 0)) continue;
          const t = va / (va - vb);
          const oa = CORNERS[a] as readonly [number, number, number];
          const ob = CORNERS[b] as readonly [number, number, number];
          const cp: Vec3 = [
            origin[0] + (i + oa[0] + (ob[0] - oa[0]) * t) * res,
            origin[1] + (j + oa[1] + (ob[1] - oa[1]) * t) * res,
            origin[2] + (k + oa[2] + (ob[2] - oa[2]) * t) * res,
          ];
          pts.push(cp);
          nrms.push(sdfNormal(field, cp, res * 0.5));
        }
        if (pts.length === 0) continue;
        const cmx = origin[0] + i * res;
        const cmy = origin[1] + j * res;
        const cmz = origin[2] + k * res;
        const raw = solveQEF(pts, nrms, [cmx + res / 2, cmy + res / 2, cmz + res / 2]);
        const v: Vec3 = [clamp(raw[0], cmx, cmx + res), clamp(raw[1], cmy, cmy + res), clamp(raw[2], cmz, cmz + res)];
        let ax = 0;
        let ay = 0;
        let az = 0;
        for (const n of nrms) {
          ax -= n[0]; // face the open interior (−gradient)
          ay -= n[1];
          az -= n[2];
        }
        const nl = Math.hypot(ax, ay, az) || 1e-6;
        let nrm: Vec3 = [ax / nl, ay / nl, az / nl];
        if (snap) nrm = quantizeNormal(nrm);
        vertIdx[ci(i, j, k)] = positions.length / 3;
        positions.push(v[0], v[1], v[2]);
        normals.push(nrm[0], nrm[1], nrm[2]);
      }

  const indices: number[] = [];
  const cellV = (i: number, j: number, k: number): number => (i >= 0 && i < dx && j >= 0 && j < dy && k >= 0 && k < dz ? (vertIdx[ci(i, j, k)] as number) : -1);
  const quad = (a: number, b: number, c: number, d: number, flip: boolean): void => {
    if (a < 0 || b < 0 || c < 0 || d < 0) return;
    if (flip) indices.push(a, c, b, a, d, c);
    else indices.push(a, b, c, a, c, d);
  };
  for (let k = 0; k < nz; k++)
    for (let j = 0; j < ny; j++)
      for (let i = 0; i < nx; i++) {
        const s = (val[gi(i, j, k)] as number) <= 0;
        if (i + 1 < nx && s !== ((val[gi(i + 1, j, k)] as number) <= 0)) quad(cellV(i, j - 1, k - 1), cellV(i, j, k - 1), cellV(i, j, k), cellV(i, j - 1, k), s);
        if (j + 1 < ny && s !== ((val[gi(i, j + 1, k)] as number) <= 0)) quad(cellV(i - 1, j, k - 1), cellV(i, j, k - 1), cellV(i, j, k), cellV(i - 1, j, k), !s);
        if (k + 1 < nz && s !== ((val[gi(i, j, k + 1)] as number) <= 0)) quad(cellV(i - 1, j - 1, k), cellV(i, j - 1, k), cellV(i, j, k), cellV(i - 1, j, k), s);
      }

  return { positions, normals, indices };
}
