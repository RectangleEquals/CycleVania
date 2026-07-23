import { describe, it, expect } from "vitest";
import { isQuantized, type Vec3, type WorldBox } from "../math/index.js";
import { GenError } from "../errors.js";
import { box, sphere, type Sdf } from "../volume/sdf.js";
import { fieldExtentsFrom, type AreaField } from "../volume/field.js";
import { dualContour } from "./mesher.js";
import { meshToKit } from "./kit.js";
import { occupancyGrid, collideSphere, isSolidAt, isWalkable } from "./occupancy.js";
import { finishArea } from "./index.js";
import type { FidelityProfile } from "./fidelity.js";

const fieldOf = (sdf: Sdf, half: Vec3, res = 1): AreaField => {
  const env: WorldBox = { min: [-half[0] - 2, -half[1] - 2, -half[2] - 2], max: [half[0] + 2, half[1] + 2, half[2] + 2] };
  const extents = fieldExtentsFrom([env], res, 1, 72);
  return { sdf, isOpen: (p) => sdf(p) < 0, extents };
};
const prof = (angleStepDeg: number | null): FidelityProfile => ({ angleStepDeg, voxelRes: 1, maxDim: 72, snapNormals: angleStepDeg !== null });

const eachNormal = (normals: number[], fn: (n: Vec3) => void): void => {
  for (let i = 0; i < normals.length; i += 3) fn([normals[i] as number, normals[i + 1] as number, normals[i + 2] as number]);
};

describe("fidelity spectrum", () => {
  it("snaps every normal to the 5° grid on a curved hull", () => {
    const mesh = dualContour(fieldOf(sphere([0, 0, 0], 5), [5, 5, 5]).sdf, fieldOf(sphere([0, 0, 0], 5), [5, 5, 5]).extents, prof(5));
    expect(mesh.normals.length).toBeGreaterThan(0);
    eachNormal(mesh.normals, (n) => expect(isQuantized(n, 5)).toBe(true));
  });

  it("produces axis-aligned normals at 90° on a box hull", () => {
    const f = fieldOf(box([0, 0, 0], [4, 4, 4]), [4, 4, 4]);
    const mesh = dualContour(f.sdf, f.extents, prof(90));
    expect(mesh.normals.length).toBeGreaterThan(0);
    eachNormal(mesh.normals, (n) => {
      const sum = Math.abs(n[0]) + Math.abs(n[1]) + Math.abs(n[2]);
      expect(sum).toBeCloseTo(1, 5); // exactly one axis component
    });
  });
});

describe("mesh sanity", () => {
  it("has no NaN, valid indices, and mostly non-degenerate triangles", () => {
    const f = fieldOf(sphere([0, 0, 0], 5), [5, 5, 5]);
    const mesh = dualContour(f.sdf, f.extents, prof(5));
    const vcount = mesh.positions.length / 3;
    for (const x of [...mesh.positions, ...mesh.normals]) expect(Number.isFinite(x)).toBe(true);
    for (const idx of mesh.indices) {
      expect(idx).toBeGreaterThanOrEqual(0);
      expect(idx).toBeLessThan(vcount);
    }
    // dual contouring can emit occasional degenerate tris at edges; assert MOST have area.
    let nonDegenerate = 0;
    const tris = mesh.indices.length / 3;
    for (let t = 0; t < mesh.indices.length; t += 3) {
      const a = mesh.indices[t]! * 3;
      const b = mesh.indices[t + 1]! * 3;
      const c = mesh.indices[t + 2]! * 3;
      const ab: Vec3 = [mesh.positions[b]! - mesh.positions[a]!, mesh.positions[b + 1]! - mesh.positions[a + 1]!, mesh.positions[b + 2]! - mesh.positions[a + 2]!];
      const ac: Vec3 = [mesh.positions[c]! - mesh.positions[a]!, mesh.positions[c + 1]! - mesh.positions[a + 1]!, mesh.positions[c + 2]! - mesh.positions[a + 2]!];
      const cross = Math.hypot(ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0]);
      if (cross > 1e-9) nonDegenerate++;
    }
    expect(nonDegenerate).toBeGreaterThan(tris * 0.5);
  });
});

describe("kit dedup + canonical yaw", () => {
  it("deduplicates heavily and records rotations", () => {
    // half-extent 4.5 keeps faces off the grid lines → clean crossings + strong repetition
    const f = fieldOf(box([0, 0, 0], [4.5, 4.5, 4.5]), [4.5, 4.5, 4.5]);
    const mesh = dualContour(f.sdf, f.extents, prof(90));
    const { kit, instances } = meshToKit(mesh, f.extents.origin, f.extents.res, { biome: "b" });
    expect(instances.length).toBeGreaterThan(0);
    expect(kit.pieces.length).toBeLessThan(instances.length / 2); // dedup effective
    // every instance yaw is a valid cardinal rotation
    const cardinals = [0, Math.PI / 2, Math.PI, (3 * Math.PI) / 2];
    for (const inst of instances) expect(cardinals.some((y) => Math.abs(inst.yaw - y) < 1e-6)).toBe(true);
  });

  it("collapses two rotation-identical cells to one piece at different yaws", () => {
    // cell (1,0,0)'s triangle is cell (0,0,0)'s triangle rotated 90° in-cell.
    const mesh = {
      positions: [0.2, 0.2, 0, 0.8, 0.2, 0, 0.5, 0.8, 0, 1.8, 0.2, 0, 1.8, 0.8, 0, 1.2, 0.5, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
      indices: [0, 1, 2, 3, 4, 5],
    };
    const { kit, instances } = meshToKit(mesh, [0, 0, 0], 1, { biome: "b" });
    expect(kit.pieces.length).toBe(1); // deduped to one canonical piece
    expect(instances.length).toBe(2);
    expect(instances[0]!.pieceId).toBe(instances[1]!.pieceId);
    expect(instances[0]!.yaw).not.toBe(instances[1]!.yaw); // different rotations
  });

  it("throws a budget GenError when too many unique pieces", () => {
    const f = fieldOf(sphere([0, 0, 0], 5), [5, 5, 5]);
    const mesh = dualContour(f.sdf, f.extents, prof(5));
    let err: GenError | undefined;
    try {
      meshToKit(mesh, f.extents.origin, f.extents.res * 2, { biome: "b", maxUniquePieces: 1 });
    } catch (e) {
      err = e as GenError;
    }
    expect(err?.code).toBe("finish.piece-budget");
  });
});

describe("occupancy + collision", () => {
  const f = fieldOf(box([0, 0, 0], [5, 5, 5]), [5, 5, 5]);
  const grid = occupancyGrid(f.sdf, f.extents.origin, f.extents.dims, f.extents.res);

  it("treats out-of-bounds as solid and interior as open", () => {
    expect(isSolidAt(grid, [0, 0, 0])).toBe(false); // inside the box (open)
    expect(isSolidAt(grid, [100, 100, 100])).toBe(true); // OOB
  });

  it("keeps an open point open and never pushes a swept sphere into solid", () => {
    const kept = collideSphere(grid, [0, 0, 0], 0.4);
    expect(isSolidAt(grid, kept)).toBe(false);
    // walk toward the +x wall repeatedly; must never end inside solid
    let p: Vec3 = [0, 0, 0];
    for (let s = 0; s < 40; s++) {
      p = [p[0] + 0.5, p[1], p[2]];
      p = collideSphere(grid, p, 0.4);
      expect(isSolidAt(grid, p)).toBe(false);
    }
  });

  it("marks a walkable cell (open with solid support below) somewhere", () => {
    let any = false;
    const [dx, dy, dz] = grid.dims;
    for (let i = 0; i < dx && !any; i++) for (let j = 0; j < dy && !any; j++) for (let k = 0; k < dz && !any; k++) if (isWalkable(grid, [i, j, k])) any = true;
    expect(any).toBe(true);
  });
});

describe("end-to-end geometry (smoke)", () => {
  it("produces a kit + occupancy per Area via worldFromRegistry with geometry on", async () => {
    const { defineRegistry } = await import("../registries/index.js");
    const { worldFromRegistry } = await import("../world/index.js");
    const reg = defineRegistry({
      gadgets: {
        capabilities: [{ id: "jump", held: "granted", facets: [{ kind: "tag", tag: "j" }], powerWeight: () => 0.5 }],
        gadgets: [{ id: "boots", grants: ["jump"] }],
      },
      gadgetEconomy: { min: 1, max: 1 },
      lengthPolicy: { min: 1, max: 1 },
      templatePool: {
        poolAt: () => [
          {
            weight: 1,
            template: {
              id: "t",
              criticalPath: ["hub", "term"],
              nodes: { hub: { role: "hub", slots: { min: 4, max: 4 } }, term: { role: "terminal", slots: { min: 1, max: 1 } } },
              branches: [],
              gating: { lockFraction: 0, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
              loops: { guaranteeAtLeastOne: false, density: 0 },
            },
          },
        ],
      },
    });
    const w = worldFromRegistry(reg, "geo-smoke", { geometry: true });
    const r = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
    const withGeo = r.skeleton.areas.filter((a) => a.finish);
    expect(withGeo.length).toBeGreaterThan(0);
    for (const a of withGeo) {
      expect(a.finish!.kit.pieces.length).toBeGreaterThan(0);
      expect(a.finish!.occupancyData.solid.length).toBeGreaterThan(0);
    }
  });
});

describe("finishArea determinism", () => {
  it("produces byte-identical geometry across runs", () => {
    const f = () => fieldOf(sphere([0, 0, 0], 5), [5, 5, 5]);
    const run = () => {
      const r = finishArea(f(), { biome: "b", fidelity: prof(5), seed: 1 });
      return JSON.stringify({ pieces: r.kit.pieces, instances: r.instances, occ: r.occupancyData, dress: r.dressing });
    };
    expect(run()).toBe(run());
  });
});
