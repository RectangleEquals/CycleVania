import { describe, it, expect } from "vitest";
import { sphere, box, union, hull, connectorTube, composeAreaField } from "./index.js";

describe("SDF primitives", () => {
  it("sphere: negative inside, positive outside, ~0 on surface", () => {
    const s = sphere([0, 0, 0], 5);
    expect(s([0, 0, 0])).toBeLessThan(0);
    expect(s([10, 0, 0])).toBeGreaterThan(0);
    expect(Math.abs(s([5, 0, 0]))).toBeLessThan(1e-6);
  });

  it("box + union", () => {
    const b = box([0, 0, 0], [2, 2, 2]);
    expect(b([0, 0, 0])).toBeLessThan(0);
    expect(b([5, 0, 0])).toBeGreaterThan(0);
    const u = union([sphere([0, 0, 0], 1), sphere([10, 0, 0], 1)]);
    expect(u([0, 0, 0])).toBeLessThan(0);
    expect(u([10, 0, 0])).toBeLessThan(0);
    expect(u([5, 0, 0])).toBeGreaterThan(0);
  });
});

describe("hull archetypes", () => {
  it("all archetypes are open (negative) at their centre", () => {
    for (const a of ["hall", "rotunda", "cavern", "shaft"]) {
      const h = hull(a, { center: [0, 0, 0], size: [12, 12, 8], seed: 1 });
      expect(h([0, 0, 0])).toBeLessThan(0);
    }
  });

  it("is deterministic", () => {
    const h1 = hull("cavern", { center: [0, 0, 0], size: [10, 10, 8], seed: 3 });
    const h2 = hull("cavern", { center: [0, 0, 0], size: [10, 10, 8], seed: 3 });
    expect(h1([1, 2, 1])).toBe(h2([1, 2, 1]));
  });
});

describe("connectors + field", () => {
  it("connector tube is open along its path", () => {
    const t = connectorTube([0, 0, 0], [1, 0, 0], [10, 0, 0], [-1, 0, 0], 1.2);
    expect(t([5, 0, 0])).toBeLessThan(0);
    expect(t([5, 8, 0])).toBeGreaterThan(0);
  });

  it("composeAreaField reports open space inside hulls + connectors", () => {
    const a = hull("rotunda", { center: [0, 0, 0], size: [10, 10, 8], seed: 2 });
    const b = hull("hall", { center: [30, 0, 0], size: [10, 10, 8], seed: 2 });
    const c = connectorTube([5, 0, 0], [1, 0, 0], [25, 0, 0], [-1, 0, 0], 1.5);
    const field = composeAreaField([a, b], [c]);
    expect(field.isOpen([0, 0, 0])).toBe(true); // room A
    expect(field.isOpen([30, 0, 0])).toBe(true); // room B
    expect(field.isOpen([15, 0, 0])).toBe(true); // corridor
    expect(field.isOpen([15, 20, 0])).toBe(false); // solid rock
  });
});
