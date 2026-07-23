import { describe, it, expect } from "vitest";
import { Rng, isQuantized, quantizeNormal, type Vec3 } from "../math/index.js";
import { sphere, box, smoothUnion, displace, hull } from "./index.js";
import { connectorTube, catmullRom } from "./spline.js";
import { resolveSocketPose } from "./socket-resolve.js";
import { composeArea } from "../skeleton/area-composer.js";
import { DEFAULT_AREA_DIALS } from "../skeleton/space-budget.js";
import { composeAreaVolume } from "./area-volume.js";

describe("SDF primitives", () => {
  it("computes signed distances", () => {
    const s = sphere([0, 0, 0], 5);
    expect(s([0, 0, 0])).toBeCloseTo(-5, 5);
    expect(s([8, 0, 0])).toBeCloseTo(3, 5);
    const b = box([0, 0, 0], [2, 2, 2]);
    expect(b([0, 0, 0])).toBeLessThan(0);
    expect(b([5, 0, 0])).toBeGreaterThan(0);
  });

  it("smoothUnion never exceeds the plain min", () => {
    const a = sphere([-3, 0, 0], 4);
    const b = sphere([3, 0, 0], 4);
    const su = smoothUnion(2, [a, b]);
    for (let x = -8; x <= 8; x += 1) {
      const p: Vec3 = [x, 0, 0];
      expect(su(p)).toBeLessThanOrEqual(Math.min(a(p), b(p)) + 1e-6);
    }
  });

  it("displace is deterministic", () => {
    const d = displace(sphere([0, 0, 0], 5), 0.5, 0.3, 7);
    expect(d([1, 2, 3])).toBe(d([1, 2, 3]));
  });
});

describe("hull archetypes", () => {
  it("open volume sits inside the envelope, solid far outside", () => {
    const h = hull("hall", { center: [0, 0, 0], size: [10, 10, 6], seed: 1, noise: 0 });
    expect(h([0, 0, 0])).toBeLessThan(0); // open at center
    expect(h([30, 0, 0])).toBeGreaterThan(0); // solid far away
  });

  it("outdoor-open stays open well above the floor", () => {
    const h = hull("outdoor-open", { center: [0, 0, 0], size: [12, 12, 6], seed: 2, noise: 0 });
    expect(h([0, 0, 4])).toBeLessThan(0); // open above the floor
  });
});

describe("spline connectors", () => {
  it("interpolates endpoints", () => {
    const pts: Vec3[] = [[0, 0, 0], [10, 0, 0]];
    expect(catmullRom(pts, 0)).toEqual([0, 0, 0]);
    expect(catmullRom(pts, 1)).toEqual([10, 0, 0]);
  });

  it("is open (negative) along the tube from end to end", () => {
    const t = connectorTube([0, 0, 0], [1, 0, 0], [12, 0, 0], [-1, 0, 0], 2);
    for (let x = 0; x <= 12; x += 2) expect(t([x, 0, 0])).toBeLessThan(0);
  });
});

describe("normal quantization", () => {
  it("snaps to the 5° and 90° grids", () => {
    const n: Vec3 = [0.71, 0.13, 0.69];
    expect(isQuantized(quantizeNormal(n, 5), 5)).toBe(true);
    const q90 = quantizeNormal([0.9, 0.2, 0.1], 90);
    // a 90° snap yields an axis-aligned normal
    const axes = q90.map((c) => Math.abs(Math.round(c)));
    expect(axes.reduce((s, c) => s + c, 0)).toBe(1);
  });
});

describe("socket resolution", () => {
  it("lands on the hull surface with an orthonormal, fidelity-snapped basis, and passable", () => {
    const h = sphere([0, 0, 0], 5); // open inside
    const pose = resolveSocketPose([8, 0, 0], [1, 0, 0], h, 5, 2);
    expect(Math.abs(h(pose.pos))).toBeLessThan(0.2); // on surface
    expect(pose.passable).toBe(true);
    // orthonormal basis
    const { forward, up, right } = pose.basis;
    const dot = (a: Vec3, b: Vec3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    expect(Math.abs(dot(forward, up))).toBeLessThan(0.05);
    expect(Math.abs(dot(forward, right))).toBeLessThan(0.05);
    expect(isQuantized(forward, 5)).toBe(true);
  });
});

describe("area volume composition", () => {
  const area = () =>
    composeArea({
      regionId: "r",
      role: "hub",
      budgetSlice: 250,
      buckets: {},
      biome: "b",
      incidentEdges: 1,
      reservedRecipes: [],
      dialCfg: DEFAULT_AREA_DIALS,
      rng: new Rng("area"),
    });

  it("resolves sockets and scatters anchors deterministically", () => {
    const run = () =>
      composeAreaVolume({ area: area(), regionLocations: [{ id: "L0", itemId: "iA" }], puzzleInstances: [], fidelityAngleStep: 5, rng: new Rng("vol") });
    const a = run();
    const b = run();
    expect(a.resolvedSockets.map((s) => s.pos)).toEqual(b.resolvedSockets.map((s) => s.pos));
    expect(a.anchors.map((x) => x.pos)).toEqual(b.anchors.map((x) => x.pos));
    expect(a.resolvedSockets.length).toBeGreaterThan(0);
    // the placed Location produced a bound gadget-pickup anchor
    expect(a.anchors.some((x) => x.binding?.type === "location" && x.binding.locationId === "L0")).toBe(true);
  });
});
