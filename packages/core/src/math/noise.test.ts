import { describe, it, expect } from "vitest";
import { gradNoise3, fbm3, noise3, fbm3v } from "./noise.js";

describe("deterministic noise", () => {
  it("stays roughly within [-1, 1] and is finite", () => {
    for (let i = 0; i < 400; i++) {
      const x = i * 0.31;
      const y = i * 0.17 - 5;
      const z = i * 0.53 + 2;
      const v = gradNoise3(x, y, z, 1234);
      expect(Number.isFinite(v)).toBe(true);
      expect(Math.abs(v)).toBeLessThan(1.2);
    }
  });

  it("is identical for the same seed and coordinates", () => {
    expect(gradNoise3(1.2, 3.4, 5.6, 42)).toBe(gradNoise3(1.2, 3.4, 5.6, 42));
    expect(noise3([1.2, 3.4, 5.6], 42)).toBe(gradNoise3(1.2, 3.4, 5.6, 42));
  });

  it("differs for a different seed", () => {
    expect(gradNoise3(1.2, 3.4, 5.6, 1)).not.toBe(gradNoise3(1.2, 3.4, 5.6, 2));
  });

  it("fbm is finite over a coarse 3D sweep and agrees with its Vec3 wrapper", () => {
    for (let ix = -3; ix <= 3; ix++) {
      for (let iy = -3; iy <= 3; iy++) {
        for (let iz = -3; iz <= 3; iz++) {
          const v = fbm3(ix * 0.7, iy * 0.7, iz * 0.7, 7, 4, 2, 0.5);
          expect(Number.isFinite(v)).toBe(true);
          expect(Math.abs(v)).toBeLessThan(1.2);
        }
      }
    }
    expect(fbm3v([0.7, -1.4, 2.1], 7)).toBe(fbm3(0.7, -1.4, 2.1, 7));
  });
});
