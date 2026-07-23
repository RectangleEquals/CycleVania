import { describe, it, expect } from "vitest";
import { dsin, dcos, datan2, reduce } from "./trig.js";

describe("deterministic trig", () => {
  it("dsin matches Math.sin within 1e-6 over a wide sweep", () => {
    for (let a = -4 * Math.PI; a <= 4 * Math.PI; a += 0.037) {
      expect(Math.abs(dsin(a) - Math.sin(a))).toBeLessThan(1e-6);
    }
  });

  it("dcos matches Math.cos within 1e-6 over a wide sweep", () => {
    for (let a = -4 * Math.PI; a <= 4 * Math.PI; a += 0.037) {
      expect(Math.abs(dcos(a) - Math.cos(a))).toBeLessThan(1e-6);
    }
  });

  it("reduce brings arguments into [-π, π]", () => {
    for (let a = -50; a <= 50; a += 0.3) {
      const r = reduce(a);
      expect(r).toBeGreaterThanOrEqual(-Math.PI - 1e-9);
      expect(r).toBeLessThanOrEqual(Math.PI + 1e-9);
    }
  });

  it("datan2 is quadrant-correct at the 8 compass points", () => {
    const cases: Array<[number, number, number]> = [
      [0, 1, 0], // +X
      [1, 1, Math.PI / 4],
      [1, 0, Math.PI / 2], // +Y
      [1, -1, (3 * Math.PI) / 4],
      [0, -1, Math.PI], // -X
      [-1, -1, -(3 * Math.PI) / 4],
      [-1, 0, -Math.PI / 2], // -Y
      [-1, 1, -Math.PI / 4],
    ];
    for (const [y, x, expected] of cases) {
      expect(Math.abs(datan2(y, x) - expected)).toBeLessThan(1e-5);
    }
    expect(datan2(0, 0)).toBe(0);
  });
});
