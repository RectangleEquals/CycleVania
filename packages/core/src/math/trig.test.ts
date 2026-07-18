import { describe, it, expect } from "vitest";
import { dsin, dcos, datan, datan2, yawFromDirection } from "./trig.js";

describe("deterministic trig", () => {
  it("dsin/dcos track Math.sin/cos within 1e-5 across the range", () => {
    for (let a = -20; a <= 20; a += 0.05) {
      expect(Math.abs(dsin(a) - Math.sin(a))).toBeLessThan(1e-5);
      expect(Math.abs(dcos(a) - Math.cos(a))).toBeLessThan(1e-5);
    }
  });

  it("datan tracks Math.atan within 1e-5", () => {
    for (let x = -20; x <= 20; x += 0.1) {
      expect(Math.abs(datan(x) - Math.atan(x))).toBeLessThan(1e-5);
    }
  });

  it("datan2 tracks Math.atan2 within 1e-5 (incl. quadrant edges)", () => {
    const vals = [-3, -1, -0.2, 0, 0.2, 1, 3];
    for (const y of vals) {
      for (const x of vals) {
        expect(Math.abs(datan2(y, x) - Math.atan2(y, x))).toBeLessThan(1e-5);
      }
    }
  });

  it("datan2(0,0) is 0 (defined, not NaN)", () => {
    expect(datan2(0, 0)).toBe(0);
  });

  it("yawFromDirection: +Y faces yaw 0, +X faces -π/2", () => {
    expect(Math.abs(yawFromDirection(0, 1))).toBeLessThan(1e-6);
    expect(Math.abs(yawFromDirection(1, 0) - -Math.PI / 2)).toBeLessThan(1e-5);
  });
});
