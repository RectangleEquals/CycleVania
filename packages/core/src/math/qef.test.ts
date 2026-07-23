import { describe, it, expect } from "vitest";
import { solveQEF } from "./qef.js";
import type { Vec3 } from "./vec.js";

describe("solveQEF", () => {
  it("returns the fallback for empty input", () => {
    const fallback: Vec3 = [9, 9, 9];
    expect(solveQEF([], [], fallback)).toEqual(fallback);
  });

  it("recovers the intersection of three orthogonal planes", () => {
    // Three axis-aligned planes meeting at q; sample points sit far apart on each.
    const q: Vec3 = [0.3, 0.4, 0.5];
    const points: Vec3[] = [
      [q[0], 10, -5],
      [7, q[1], 2],
      [-3, 8, q[2]],
    ];
    const normals: Vec3[] = [
      [1, 0, 0],
      [0, 1, 0],
      [0, 0, 1],
    ];
    const v = solveQEF(points, normals, [0, 0, 0]);
    expect(Math.abs(v[0] - q[0])).toBeLessThan(0.1);
    expect(Math.abs(v[1] - q[1])).toBeLessThan(0.1);
    expect(Math.abs(v[2] - q[2])).toBeLessThan(0.1);
  });

  it("stays finite and near the mass point for a degenerate (parallel-plane) system", () => {
    // Three planes sharing one normal — under-constrained; regularization pulls
    // the solution toward the mass point without producing NaNs.
    const points: Vec3[] = [
      [1, 0, 0],
      [3, 5, -2],
      [5, -4, 8],
    ];
    const normals: Vec3[] = [
      [1, 0, 0],
      [1, 0, 0],
      [1, 0, 0],
    ];
    const mass: Vec3 = [3, 1 / 3, 2];
    const v = solveQEF(points, normals, [0, 0, 0]);
    expect(Number.isFinite(v[0])).toBe(true);
    expect(Number.isFinite(v[1])).toBe(true);
    expect(Number.isFinite(v[2])).toBe(true);
    // y and z are unconstrained → collapse to the mass point.
    expect(Math.abs(v[1] - mass[1])).toBeLessThan(0.1);
    expect(Math.abs(v[2] - mass[2])).toBeLessThan(0.1);
  });
});
