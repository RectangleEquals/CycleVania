/**
 * QEF (Quadric Error Function) solver for dual contouring. Given hermite data —
 * surface intersection points `p_i` with normals `n_i` — find the vertex that
 * minimises Σ (n_i · (x − p_i))². Solved as a regularised 3×3 least-squares
 * (deterministic Gaussian elimination), falling back to the mass point when the
 * system is degenerate (flat regions). Pure arithmetic → engine-identical.
 */

import type { Vec3 } from "./vec.js";

/** Solve a 3×3 linear system A·x = b by Gaussian elimination with partial pivoting. */
function solve3(A: number[][], b: number[]): [number, number, number] | null {
  const M = [
    [A[0]![0]!, A[0]![1]!, A[0]![2]!, b[0]!],
    [A[1]![0]!, A[1]![1]!, A[1]![2]!, b[1]!],
    [A[2]![0]!, A[2]![1]!, A[2]![2]!, b[2]!],
  ];
  for (let col = 0; col < 3; col++) {
    let piv = col;
    for (let r = col + 1; r < 3; r++) if (Math.abs(M[r]![col]!) > Math.abs(M[piv]![col]!)) piv = r;
    if (Math.abs(M[piv]![col]!) < 1e-10) return null;
    const tmp = M[col]!;
    M[col] = M[piv]!;
    M[piv] = tmp;
    for (let r = 0; r < 3; r++) {
      if (r === col) continue;
      const f = M[r]![col]! / M[col]![col]!;
      for (let c = col; c < 4; c++) M[r]![c]! -= f * M[col]![c]!;
    }
  }
  return [M[0]![3]! / M[0]![0]!, M[1]![3]! / M[1]![1]!, M[2]![3]! / M[2]![2]!];
}

/** Minimise the QEF; returns a vertex position (relative to the same space as the inputs). */
export function solveQEF(points: readonly Vec3[], normals: readonly Vec3[], fallback: Vec3, regularization = 0.01): Vec3 {
  const n = points.length;
  if (n === 0) return fallback;
  let mx = 0;
  let my = 0;
  let mz = 0;
  for (const p of points) {
    mx += p[0];
    my += p[1];
    mz += p[2];
  }
  mx /= n;
  my /= n;
  mz /= n;

  // Build AᵀA (symmetric) and Aᵀb, relative to the mass point for stability.
  let a00 = 0;
  let a01 = 0;
  let a02 = 0;
  let a11 = 0;
  let a12 = 0;
  let a22 = 0;
  let b0 = 0;
  let b1 = 0;
  let b2 = 0;
  for (let i = 0; i < n; i++) {
    const nrm = normals[i] as Vec3;
    const pt = points[i] as Vec3;
    const nx = nrm[0];
    const ny = nrm[1];
    const nz = nrm[2];
    const px = pt[0] - mx;
    const py = pt[1] - my;
    const pz = pt[2] - mz;
    const d = nx * px + ny * py + nz * pz;
    a00 += nx * nx;
    a01 += nx * ny;
    a02 += nx * nz;
    a11 += ny * ny;
    a12 += ny * nz;
    a22 += nz * nz;
    b0 += nx * d;
    b1 += ny * d;
    b2 += nz * d;
  }
  a00 += regularization;
  a11 += regularization;
  a22 += regularization;

  const s = solve3([[a00, a01, a02], [a01, a11, a12], [a02, a12, a22]], [b0, b1, b2]);
  if (!s) return [mx, my, mz];
  return [mx + s[0], my + s[1], mz + s[2]];
}
