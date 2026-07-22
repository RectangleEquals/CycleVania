import { describe, it, expect } from "vitest";
import { hull, connectorTube, composeAreaField } from "../volume/index.js";
import { dualContour, meshToKit, occupancyGrid, collideSphere, isSolidAt, quantizeNormal, isQuantized } from "./index.js";
import type { Vec3 } from "../math/vec.js";
import type { Coord } from "../spatial/grid.js";

// A small two-room + corridor area field for the whole geometry pipeline.
function demoField() {
  const a = hull("rotunda", { center: [8, 8, 6], size: [10, 10, 8], seed: 5 });
  const b = hull("hall", { center: [30, 8, 6], size: [12, 8, 8], seed: 6 });
  const c = connectorTube([13, 8, 6], [1, 0, 0], [24, 8, 6], [-1, 0, 0], 1.6);
  return composeAreaField([a, b], [c]);
}
const ORIGIN: Vec3 = [0, 0, 0];
const DIMS: Coord = [40, 16, 12];
const RES = 1;

describe("fidelity", () => {
  it("quantizes normals onto the 5° grid", () => {
    for (const n of [[0.13, 0.71, 0.02], [1, 1, 1], [-0.3, 0.4, 0.86], [0, 0, 1]] as Vec3[]) {
      const q = quantizeNormal(n);
      expect(Math.hypot(q[0], q[1], q[2])).toBeCloseTo(1, 5);
      expect(isQuantized(q)).toBe(true); // idempotent
    }
  });
});

describe("dual contouring", () => {
  const mesh = dualContour(demoField().sdf, ORIGIN, DIMS, RES);
  it("produces a finite, non-empty mesh", () => {
    expect(mesh.positions.length).toBeGreaterThan(60);
    expect(mesh.indices.length).toBeGreaterThan(60);
    expect(mesh.positions.every(Number.isFinite)).toBe(true);
    expect(mesh.normals.every(Number.isFinite)).toBe(true);
  });
  it("every normal lies on the 5° grid", () => {
    for (let i = 0; i < mesh.normals.length; i += 3) {
      expect(isQuantized([mesh.normals[i]!, mesh.normals[i + 1]!, mesh.normals[i + 2]!], 1e-2)).toBe(true);
    }
  });
  it("is deterministic (same seed → identical buffers)", () => {
    const m2 = dualContour(demoField().sdf, ORIGIN, DIMS, RES);
    expect(m2.positions).toEqual(mesh.positions);
    expect(m2.indices).toEqual(mesh.indices);
  });
});

describe("kit dedup", () => {
  it("emits far fewer unique pieces than instances", () => {
    const mesh = dualContour(demoField().sdf, ORIGIN, DIMS, RES);
    const { kit, instances } = meshToKit(mesh, ORIGIN, RES, { biome: "verdant" });
    expect(instances.length).toBeGreaterThan(0);
    expect(kit.pieces.length).toBeLessThan(instances.length);
    expect(kit.pieces.some((p) => p.meta.surface === "floor")).toBe(true);
    // every instance references an existing piece
    const ids = new Set(kit.pieces.map((p) => p.id));
    expect(instances.every((i) => ids.has(i.pieceId))).toBe(true);
  });
});

describe("occupancy + collision", () => {
  const grid = occupancyGrid(demoField().sdf, ORIGIN, DIMS, RES);
  it("marks room interiors open and rock solid", () => {
    expect(isSolidAt(grid, [8, 8, 6])).toBe(false); // room A centre
    expect(isSolidAt(grid, [30, 8, 6])).toBe(false); // room B centre
    expect(isSolidAt(grid, [8, 8, 20])).toBe(true); // far above
  });
  it("pushes a sphere out of solid rock and leaves open space alone", () => {
    const free = collideSphere(grid, [8, 8, 6], 0.4);
    expect(free[0]).toBeCloseTo(8);
    // find an OPEN cell that borders solid rock (a real wall next to walkable space)
    const [dx, dy, dz] = DIMS;
    const solidAt = (i: number, j: number, k: number): boolean => grid.solid[(k * dy + j) * dx + i] === 1;
    let openNearWall: Vec3 | null = null;
    for (let k = 1; k < dz - 1 && !openNearWall; k++)
      for (let j = 1; j < dy - 1 && !openNearWall; j++)
        for (let i = 1; i < dx - 1 && !openNearWall; i++)
          if (!solidAt(i, j, k) && (solidAt(i - 1, j, k) || solidAt(i + 1, j, k) || solidAt(i, j - 1, k) || solidAt(i, j + 1, k)))
            openNearWall = [i + 0.5, j + 0.5, k + 0.5];
    expect(openNearWall).not.toBeNull();
    // a sphere in that open cell overlapping the wall gets pushed back but stays in open space
    const pushed = collideSphere(grid, openNearWall as Vec3, 0.6);
    expect(isSolidAt(grid, pushed as Vec3)).toBe(false); // never penetrates the wall
  });
});
