import { describe, it, expect } from "vitest";
import { generateReach } from "./grammar.js";
import type { ReachTemplate } from "./template.js";
import type { ProgressionItem } from "../graph/region-graph.js";
import { isSolvable, hasCycle } from "../graph/solvable.js";

const TEMPLATE: ReachTemplate = {
  criticalPath: ["hub", "seg1", "seg2", "seg3", "seg4", "seg5", "capstone", "terminal"],
  nodes: {
    hub: { id: "hub", role: "hub", slots: { min: 3, max: 4 }, bootstrap: true },
    seg1: { id: "seg1", role: "segment", slots: { min: 1, max: 2 } },
    seg2: { id: "seg2", role: "segment", slots: { min: 1, max: 2 } },
    seg3: { id: "seg3", role: "segment", slots: { min: 1, max: 2 } },
    seg4: { id: "seg4", role: "segment", slots: { min: 1, max: 2 } },
    seg5: { id: "seg5", role: "segment", slots: { min: 1, max: 2 } },
    capstone: { id: "capstone", role: "capstone", slots: { min: 1, max: 1 } },
    terminal: { id: "terminal", role: "terminal", slots: { min: 1, max: 1 } },
  },
  branches: [
    { anchor: "any-segment", role: "vault", slots: { min: 1, max: 2 }, entrance: "compound", backEdge: { chance: 0.6, toEarlier: true, gated: 0.5 } },
  ],
  gating: { lockFraction: 0.5, compoundChance: 0.25, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true },
};

const ITEMS: ProgressionItem[] = [
  { id: "tether", grants: "tether" },
  { id: "impeller", grants: "impeller" },
  { id: "lantern", grants: "reveal" },
];

describe("generateReach", () => {
  it("produces a solvable Reach across many seeds", () => {
    for (let s = 0; s < 400; s++) {
      const r = generateReach({ seed: `r-${s}`, template: TEMPLATE, items: ITEMS });
      const byId = new Map(r.items.map((i) => [i.id, i]));
      expect(isSolvable(r.graph, r.placement, byId, r.startCaps)).toBe(true);
    }
  });

  it("guarantees a backtracking cycle and produces gated edges", () => {
    const r = generateReach({ seed: "loops", template: TEMPLATE, items: ITEMS });
    expect(hasCycle(r.graph)).toBe(true);
    expect(r.meta.gatedEdges.length).toBeGreaterThan(0);
    expect(r.meta.spine.length).toBe(5);
  });

  it("is deterministic", () => {
    const a = generateReach({ seed: "det", template: TEMPLATE, items: ITEMS });
    const b = generateReach({ seed: "det", template: TEMPLATE, items: ITEMS });
    expect([...a.placement.entries()].sort()).toEqual([...b.placement.entries()].sort());
    expect(a.graph.edges.length).toBe(b.graph.edges.length);
  });
});
