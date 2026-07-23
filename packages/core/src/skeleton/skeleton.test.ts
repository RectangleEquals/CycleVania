import { describe, it, expect } from "vitest";
import { have, ALWAYS, type Rule } from "../logic/index.js";
import { length as vlen, sub } from "../math/index.js";
import type { MissionGraph, Region, NodeRole } from "../graph/index.js";
import { buildReachSkeleton } from "./reach-skeleton.js";
import { deriveAreaDials, DEFAULT_AREA_DIALS } from "./space-budget.js";
import { composeArea } from "./area-composer.js";
import { spaceRadius } from "./space-plan.js";
import { Rng } from "../math/index.js";

const R = (id: string, role: NodeRole = "segment"): Region => ({ id, role });
const chain = (n: number): MissionGraph => {
  const regions: Region[] = [R("r0", "hub"), ...Array.from({ length: n - 2 }, (_, i) => R(`r${i + 1}`)), R(`r${n - 1}`, "capstone")];
  const edges = Array.from({ length: n - 1 }, (_, i) => ({ from: `r${i}`, to: `r${i + 1}`, rule: ALWAYS }));
  return { regions, edges, flags: [], locations: [], start: "r0" };
};

describe("skeleton — layout", () => {
  it("keeps intra-area Space bounding volumes from overlapping", () => {
    for (let seed = 0; seed < 20; seed++) {
      const sk = buildReachSkeleton(chain(6), `ov-${seed}`, { finalCeiling: 300, buckets: {} });
      for (const a of sk.areas) {
        for (let i = 0; i < a.spaces.length; i++) {
          for (let j = i + 1; j < a.spaces.length; j++) {
            const si = a.spaces[i]!;
            const sj = a.spaces[j]!;
            const d = vlen(sub(sj.origin, si.origin));
            const minSep = (spaceRadius(si.budget.volumeCells) + spaceRadius(sj.budget.volumeCells)) * 0.9;
            expect(d).toBeGreaterThanOrEqual(minSep);
          }
        }
      }
    }
  });

  it("is deterministic for a given seed", () => {
    const key = (): string => JSON.stringify(buildReachSkeleton(chain(5), "det", { finalCeiling: 250, buckets: {} }).areas.map((a) => [a.regionId, a.spaces.map((s) => s.origin), a.connectors.length]));
    expect(key()).toBe(key());
  });
});

describe("skeleton — capacity + junctions", () => {
  it("inserts junctions and wires every incident edge for a dense hub", () => {
    const regions: Region[] = [R("hub", "hub"), ...Array.from({ length: 8 }, (_, i) => R(`leaf${i}`))];
    const edges = Array.from({ length: 8 }, (_, i) => ({ from: "hub", to: `leaf${i}`, rule: ALWAYS }));
    const g: MissionGraph = { regions, edges, flags: [], locations: [], start: "hub" };
    const sk = buildReachSkeleton(g, "cap", { finalCeiling: 30, buckets: {} }); // tiny budget → 1 space/area
    const hub = sk.areas.find((a) => a.regionId === "hub")!;
    expect(hub.relaxations).toContain("skeleton.junction-inserted");
    expect(hub.spaces.some((s) => s.role === "junction")).toBe(true);
    expect(sk.interAreaConnectors.length).toBe(edges.length); // no edge dropped
    expect(sk.relaxations).not.toContain("skeleton.inter-area-socket-missing");
  });
});

describe("skeleton — L2 decisions (no geometry)", () => {
  it("decides outdoor Spaces at L2", () => {
    const cfg = { ...DEFAULT_AREA_DIALS, baseOutdoorChance: 1, baseLargeChance: 1 };
    let sawOutdoor = false;
    for (let seed = 0; seed < 10 && !sawOutdoor; seed++) {
      const sk = buildReachSkeleton(chain(6), `out-${seed}`, { finalCeiling: 400, buckets: {}, dialCfg: cfg });
      if (sk.areas.some((a) => a.spaces.some((s) => s.outdoor))) sawOutdoor = true;
    }
    expect(sawOutdoor).toBe(true);
  });

  it("flags 1–2 landmark Spaces per Reach", () => {
    const sk = buildReachSkeleton(chain(6), "lm", { finalCeiling: 300, buckets: {} });
    expect(sk.landmarkSpaceIds.length).toBeGreaterThanOrEqual(1);
    expect(sk.landmarkSpaceIds.length).toBeLessThanOrEqual(2);
    const flagged = sk.areas.flatMap((a) => a.spaces).filter((s) => s.landmark).map((s) => s.id).sort();
    expect(flagged).toEqual([...sk.landmarkSpaceIds].sort());
  });
});

describe("skeleton — gate fidelity + z-plan", () => {
  it("carries an L1 edge's gate Rule onto the inter-area connector BY REFERENCE", () => {
    const gate: Rule = have("grapple");
    const g: MissionGraph = {
      regions: [R("a", "hub"), R("b")],
      edges: [{ from: "a", to: "b", rule: gate }],
      flags: [],
      locations: [],
      start: "a",
    };
    const sk = buildReachSkeleton(g, "gf", { finalCeiling: 200, buckets: {} });
    expect(sk.interAreaConnectors.some((c) => c.gate === gate)).toBe(true); // === identity
  });

  it("places a one-way (drop) edge's target Area strictly lower", () => {
    const g: MissionGraph = {
      regions: [R("top", "hub"), R("bottom")],
      edges: [{ from: "top", to: "bottom", rule: ALWAYS, oneWay: true }],
      flags: [],
      locations: [],
      start: "top",
    };
    const sk = buildReachSkeleton(g, "z", { finalCeiling: 200, buckets: {} });
    expect(sk.areaOrigins["bottom"]![2]).toBeLessThan(sk.areaOrigins["top"]![2]);
  });

  it("zSpread responds to the zUp bucket with a sqrt shape", () => {
    const rng = new Rng("z");
    const z0 = deriveAreaDials(200, {}, "segment", DEFAULT_AREA_DIALS, rng.fork("a")).zSpread;
    const z4 = deriveAreaDials(200, { "traversal.zUp": 4 }, "segment", DEFAULT_AREA_DIALS, rng.fork("a")).zSpread;
    const z16 = deriveAreaDials(200, { "traversal.zUp": 16 }, "segment", DEFAULT_AREA_DIALS, rng.fork("a")).zSpread;
    // sqrt(16)/sqrt(4) = 2 → (z16 - base)/(z4 - base) ≈ 2
    const base = DEFAULT_AREA_DIALS.zBase;
    expect((z16 - base) / (z4 - base)).toBeCloseTo(2, 1);
    expect(z4).toBeGreaterThan(z0);
  });
});

describe("skeleton — connector length bounds + waypoints", () => {
  it("uses per-traversal length bounds and inserts a waypoint when a connector is too long", () => {
    // huge budget + few spaces → large radii → connector length exceeds bounds → waypoint
    const cfg = { ...DEFAULT_AREA_DIALS, maxSpaces: 3, minSpaces: 3, spaceCost: 1 };
    const area = composeArea({
      regionId: "big",
      role: "segment",
      budgetSlice: 30000,
      buckets: {},
      biome: "b",
      incidentEdges: 0,
      reservedRecipes: [],
      dialCfg: cfg,
      rng: new Rng("wp"),
    });
    expect(area.connectors.some((c) => c.waypoints && c.waypoints.length > 0)).toBe(true);
    // every connector's bounds match its traversal
    for (const c of area.connectors) {
      expect(c.lengthBounds.min).toBeGreaterThan(0);
      expect(c.lengthBounds.max).toBeGreaterThan(c.lengthBounds.min);
    }
  });
});
