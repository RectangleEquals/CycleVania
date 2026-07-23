import { describe, it, expect } from "vitest";
import { have } from "../logic/index.js";
import type { Item } from "../graph/index.js";
import type { ReachTemplate, ReachTemplatePool } from "../template/index.js";
import { createWorld, verbatimSelector, type ReachResult, type WorldConfig } from "./world-composer.js";
import { computeVirtualSchedule, type SchedulableEntry } from "./virtual-schedule.js";

const demoTemplate: ReachTemplate = {
  id: "p",
  criticalPath: ["hub", "s1", "s2", "capstone", "terminal"],
  nodes: {
    hub: { role: "hub", slots: { min: 6, max: 6 } },
    s1: { role: "segment", slots: { min: 2, max: 2 } },
    s2: { role: "segment", slots: { min: 2, max: 2 } },
    capstone: { role: "capstone", slots: { min: 1, max: 1 } },
    terminal: { role: "terminal", slots: { min: 1, max: 1 } },
  },
  branches: [],
  gating: { lockFraction: 0.5, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true, density: 0 },
};
const pool: ReachTemplatePool = { poolAt: () => [{ template: demoTemplate, weight: 1 }] };
const items: Item[] = [
  { id: "iA", class: "progression", grants: ["capA"] },
  { id: "iB", class: "progression", grants: ["capB"] },
];
const config = (seed: string): WorldConfig => ({ seed, templatePool: pool, selector: verbatimSelector(items, [have("capA")]) });

const canon = (r: ReachResult): string => JSON.stringify({ meta: r.meta, placement: [...r.placement.entries()].sort() });

describe("preview purity", () => {
  it("previewing never perturbs subsequent generation", () => {
    const wA = createWorld(config("purity"));
    for (let k = 0; k < 5; k++) wA.previewReachEnvelope(3);
    const a = [0, 1, 2, 3].map((i) =>
      wA.requestReach(i === 0 ? { reachIndex: 0, chosenModifiers: [] } : { reachIndex: i, fromReachIndex: i - 1, chosenModifiers: [] }),
    );
    const wB = createWorld(config("purity")); // never previews
    const b = [0, 1, 2, 3].map((i) =>
      wB.requestReach(i === 0 ? { reachIndex: 0, chosenModifiers: [] } : { reachIndex: i, fromReachIndex: i - 1, chosenModifiers: [] }),
    );
    for (let i = 0; i < 4; i++) expect(canon(a[i] as ReachResult)).toBe(canon(b[i] as ReachResult));
  });

  it("previewReachEnvelope is idempotent", () => {
    const w = createWorld({ ...config("idem"), lengthPolicy: { min: 4, max: 6 } });
    expect(JSON.stringify(w.previewReachEnvelope(5))).toBe(JSON.stringify(w.previewReachEnvelope(5)));
  });
});

describe("virtual schedule", () => {
  const gadgets: SchedulableEntry[] = Array.from({ length: 10 }, (_, k) => ({
    id: `g${k}`,
    powerWeight: () => k / 9, // g0 lowest power … g9 highest
  }));

  it("is pure, covers every entry, and spreads across 0..L-1", () => {
    const L = 5;
    const a = computeVirtualSchedule("vs", L, gadgets, "virtual-schedule:gadgets");
    const b = computeVirtualSchedule("vs", L, gadgets, "virtual-schedule:gadgets");
    expect([...a.entries()].sort()).toEqual([...b.entries()].sort()); // pure
    expect(a.size).toBe(gadgets.length); // covers all
    for (const slot of a.values()) {
      expect(slot).toBeGreaterThanOrEqual(0);
      expect(slot).toBeLessThan(L);
    }
    // low-power tends earlier than high-power
    const g0 = a.get("g0") ?? 0;
    const g9 = a.get("g9") ?? 0;
    expect(g0).toBeLessThanOrEqual(g9);
  });

  it("uses independent entropy per namespace", () => {
    const g = computeVirtualSchedule("vs", 6, gadgets, "virtual-schedule:gadgets");
    const p = computeVirtualSchedule("vs", 6, gadgets, "virtual-schedule:puzzles");
    // Same base spread, but the jitter streams differ → not identical maps.
    expect(JSON.stringify([...g])).not.toBe(JSON.stringify([...p]));
  });
});
