import { describe, it, expect } from "vitest";
import { gradNoise3, fbm3 } from "./noise.js";
import { solveQEF } from "./qef.js";

describe("gradient noise", () => {
  it("is deterministic for the same coords + seed", () => {
    expect(gradNoise3(1.5, 2.5, 3.5, 7)).toBe(gradNoise3(1.5, 2.5, 3.5, 7));
    expect(fbm3(0.3, 0.7, 1.1, 42)).toBe(fbm3(0.3, 0.7, 1.1, 42));
  });

  it("varies with position and seed, and stays roughly in [-1,1]", () => {
    expect(gradNoise3(1.5, 2.5, 3.5, 7)).not.toBe(gradNoise3(1.6, 2.5, 3.5, 7));
    expect(gradNoise3(1.5, 2.5, 3.5, 7)).not.toBe(gradNoise3(1.5, 2.5, 3.5, 8));
    for (let i = 0; i < 500; i++) {
      const v = fbm3(i * 0.13, i * 0.07, i * 0.19, 3);
      expect(v).toBeGreaterThanOrEqual(-1.2);
      expect(v).toBeLessThanOrEqual(1.2);
    }
  });

  it("is zero at lattice points (gradient noise property)", () => {
    expect(Math.abs(gradNoise3(4, 5, 6, 1))).toBeLessThan(1e-9);
  });
});

describe("QEF solver", () => {
  it("finds the corner where three orthogonal planes meet", () => {
    // three planes through points with axis normals → intersection at (2,3,4)
    const v = solveQEF(
      [
        [2, 0, 0],
        [0, 3, 0],
        [0, 0, 4],
      ],
      [
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
      ],
      [0, 0, 0],
    );
    expect(Math.abs(v[0] - 2)).toBeLessThan(0.1);
    expect(Math.abs(v[1] - 3)).toBeLessThan(0.1);
    expect(Math.abs(v[2] - 4)).toBeLessThan(0.1);
  });

  it("falls back to the mass point on degenerate (parallel) input", () => {
    const v = solveQEF(
      [
        [0, 0, 0],
        [2, 0, 0],
      ],
      [
        [1, 0, 0],
        [1, 0, 0],
      ],
      [-9, -9, -9],
    );
    // planes are parallel (x=0 and x=2 both normal +x); regularised solve stays near the mass point
    expect(v[1]).toBeCloseTo(0, 5);
    expect(v[2]).toBeCloseTo(0, 5);
  });

  it("is deterministic", () => {
    const args = [
      [[1, 1, 1], [3, 1, 1]] as const,
      [[1, 0, 0], [1, 0, 0]] as const,
    ] as const;
    const a = solveQEF(args[0], args[1], [0, 0, 0]);
    const b = solveQEF(args[0], args[1], [0, 0, 0]);
    expect(a).toEqual(b);
  });
});
