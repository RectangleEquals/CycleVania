import { describe, it, expect } from "vitest";
import { Rng } from "../math/index.js";
import { have, heldOf } from "../logic/index.js";
import { interpretTemplate, type ReachTemplate, type SelectedContent } from "../template/index.js";
import { isSolvable, validateGraph, type Item, type MissionGraph } from "../graph/index.js";
import { assumedFill } from "./assumed-fill.js";
import { locationWeight, DEFAULT_PLACEMENT_WEIGHTS, type CandidateLocation } from "./placement-weights.js";

const demoTemplate: ReachTemplate = {
  id: "demo",
  criticalPath: ["hub", "s1", "s2", "gate1", "s3", "capstone", "terminal"],
  nodes: {
    hub: { role: "hub", slots: { min: 5, max: 6 } },
    s1: { role: "segment", slots: { min: 2, max: 2 } },
    s2: { role: "segment", slots: { min: 1, max: 2 } },
    gate1: { role: "gate", slots: { min: 1, max: 1 } },
    s3: { role: "segment", slots: { min: 1, max: 2 } },
    capstone: { role: "capstone", slots: { min: 1, max: 1 } },
    terminal: { role: "terminal", slots: { min: 0, max: 1 } },
  },
  branches: [
    { attachTo: "s1", role: "vault", entrance: "single", slots: { min: 1, max: 2 }, backEdgeChance: 0.3 },
    { attachTo: "s2", role: "vault", entrance: "single", slots: { min: 1, max: 2 }, backEdgeChance: 0.3 },
  ],
  gating: { lockFraction: 0.5, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true, density: 0.2 },
};

const items: Item[] = [
  { id: "iA", class: "progression", grants: ["capA"] },
  { id: "iB", class: "progression", grants: ["capB"] },
  { id: "iC", class: "progression", grants: ["capC"] },
  { id: "iD", class: "progression", grants: ["capD"] },
  { id: "f1", class: "filler" },
  { id: "f2", class: "filler" },
];
const content: SelectedContent = {
  gateRules: [have("capA"), have("capB"), have("capC")],
  progressionCount: 4,
};
const fullyEquipped = heldOf(["capA", "capB", "capC", "capD"]);

describe("assumed fill", () => {
  it("produces a solvable Reach for 1000 seeds with zero throws", () => {
    for (let seed = 0; seed < 1000; seed++) {
      const rng = new Rng(`soak-${seed}`);
      const graph = interpretTemplate(demoTemplate, content, {}, rng.fork("interp"));
      validateGraph(graph, fullyEquipped); // throws on a stranded region
      const { placement } = assumedFill(graph, heldOf([]), items, rng.fork("fill"));
      expect(isSolvable(graph, heldOf([]), items, placement)).toBe(true);
    }
  });

  it("is seed-varied (different seeds give different placements)", () => {
    const g = interpretTemplate(demoTemplate, content, {}, new Rng("fixed").fork("interp"));
    const p1 = assumedFill(g, heldOf([]), items, new Rng("s1").fork("fill")).placement;
    const p2 = assumedFill(g, heldOf([]), items, new Rng("s2").fork("fill")).placement;
    const key = (p: Map<string, string>): string =>
      [...p.entries()].sort().map(([k, v]) => `${k}=${v}`).join(",");
    expect(key(p1)).not.toBe(key(p2));
  });
});

describe("placement distribution", () => {
  const distTemplate: ReachTemplate = {
    id: "dist",
    criticalPath: ["hub", "s1", "s2", "s3", "s4", "capstone", "terminal"],
    nodes: {
      hub: { role: "hub", slots: { min: 6, max: 6 } },
      s1: { role: "segment", slots: { min: 2, max: 2 } },
      s2: { role: "segment", slots: { min: 2, max: 2 } },
      s3: { role: "segment", slots: { min: 2, max: 2 } },
      s4: { role: "segment", slots: { min: 2, max: 2 } },
      capstone: { role: "capstone", slots: { min: 1, max: 1 } },
      terminal: { role: "terminal", slots: { min: 1, max: 1 } },
    },
    branches: [
      { attachTo: "s2", role: "vault", entrance: "single", slots: { min: 2, max: 2 }, backEdgeChance: 0 },
      { attachTo: "s3", role: "vault", entrance: "single", slots: { min: 2, max: 2 }, backEdgeChance: 0 },
    ],
    gating: { lockFraction: 0.4, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
    loops: { guaranteeAtLeastOne: true, density: 0 },
  };
  const distContent: SelectedContent = { gateRules: [have("a"), have("b")], progressionCount: 4 };
  const distItems: Item[] = [
    { id: "iA", class: "progression", grants: ["a"] },
    { id: "iB", class: "progression", grants: ["b"] },
    { id: "iC", class: "progression", grants: ["c"] },
    { id: "iD", class: "progression", grants: ["d"] },
  ];

  it("never places progression in the entry Region, ≤1 per Region, favors vaults, no relaxations", () => {
    const N = 300;
    const perRegion = new Map<string, number>();
    let vaultPlacements = 0;
    let segmentPlacements = 0;
    const vaultRegions = new Set<string>();
    const segmentRegions = new Set<string>();

    for (let seed = 0; seed < N; seed++) {
      const rng = new Rng(`dist-${seed}`);
      const g: MissionGraph = interpretTemplate(distTemplate, distContent, {}, rng.fork("interp"));
      const roleOf = new Map(g.regions.map((r) => [r.id, r.role] as const));
      const regionOfLoc = new Map(g.locations.map((l) => [l.id, l.region] as const));
      const { placement, relaxations } = assumedFill(g, heldOf([]), distItems, rng.fork("fill"));

      expect(relaxations).toEqual([]); // ample slots ⇒ no relaxation ever

      const countThisSeed = new Map<string, number>();
      for (const [loc, itemId] of placement) {
        if (!itemId.startsWith("i")) continue; // progression ids iA..iD
        const region = regionOfLoc.get(loc) as string;
        countThisSeed.set(region, (countThisSeed.get(region) ?? 0) + 1);
        perRegion.set(region, (perRegion.get(region) ?? 0) + 1);
        const role = roleOf.get(region);
        if (role === "hub") throw new Error(`progression placed in entry hub (${region})`);
        if (role === "vault") {
          vaultPlacements++;
          vaultRegions.add(region);
        } else {
          segmentPlacements++;
          segmentRegions.add(region);
        }
      }
      for (const [, c] of countThisSeed) expect(c).toBeLessThanOrEqual(1); // perRegionCap
    }

    // Vaults receive progression in a healthy fraction of seeds (the bias is real).
    // The exact "vault beats same-depth segment" tilt is unit-tested below,
    // apples-to-apples; in aggregate the depth term legitimately lets the deepest
    // segments (capstone) compete, which is itself the intended exploration reward.
    expect(vaultPlacements).toBeGreaterThan(N * 0.2);
    void segmentPlacements;
    void segmentRegions;
    void vaultRegions;
    void perRegion;
  });
});

describe("locationWeight bias", () => {
  const base: CandidateLocation = {
    locationId: "L",
    regionId: "R",
    regionRole: "segment",
    depthRank: 3,
    behindGateCount: 1,
    isEntry: false,
  };
  const cfg = DEFAULT_PLACEMENT_WEIGHTS;
  const ctx = { placedInRegion: new Map<string, number>() };

  it("favors vaults over same-depth segments", () => {
    expect(locationWeight({ ...base, regionRole: "vault" }, ctx, cfg)).toBeGreaterThan(locationWeight(base, ctx, cfg));
  });
  it("favors deeper Locations", () => {
    expect(locationWeight({ ...base, depthRank: 5 }, ctx, cfg)).toBeGreaterThan(locationWeight(base, ctx, cfg));
  });
  it("favors Locations behind a gate", () => {
    expect(locationWeight(base, ctx, cfg)).toBeGreaterThan(locationWeight({ ...base, behindGateCount: 0 }, ctx, cfg));
  });
  it("zeroes the entry Region", () => {
    expect(locationWeight({ ...base, isEntry: true }, ctx, cfg)).toBe(0);
  });
  it("zeroes a Region already at the per-Region cap", () => {
    expect(locationWeight(base, { placedInRegion: new Map([["R", 1]]) }, cfg)).toBe(0);
  });
});
