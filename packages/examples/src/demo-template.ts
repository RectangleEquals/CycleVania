/**
 * The default CrawlStar-style Reach template: a hub + 5 segments + capstone +
 * terminal on the critical path, vault branches with compound entrances and
 * back-edge loops. Games can declare any shape; this reproduces the classic
 * cadence for parity and demos.
 */

import type { ReachTemplate } from "@cyclevania/core";

export const demoTemplate: ReachTemplate = {
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
    {
      anchor: "any-segment",
      role: "vault",
      slots: { min: 1, max: 2 },
      entrance: "compound",
      count: { min: 1, max: 3 },
      backEdge: { chance: 0.6, toEarlier: true, gated: 0.5 },
    },
  ],
  gating: { lockFraction: 0.5, compoundChance: 0.25, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true },
};
