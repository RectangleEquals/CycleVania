import { describe, it, expect } from "vitest";
import { defineRegistry } from "../registries/registry.js";
import type { GeometryKit } from "../registries/geometry-kit.js";
import type { ReachTemplate } from "../template/template.js";
import { composeWorld, type ComposeContext } from "../composers/index.js";
import { composeWorldAsync, composeReachAsync } from "./orchestrator.js";
import { CancellationToken, GenCancelled } from "./cancellation.js";
import { GenerationHorizon } from "./horizon.js";

const KIT: GeometryKit = {
  pieces: [
    { id: "floor", role: "floor", snapAngles: [0], tags: [] },
    { id: "ceil", role: "ceiling", snapAngles: [0], tags: [] },
    { id: "wall", role: "wall", snapAngles: [0], tags: [] },
    { id: "corner", role: "corner", snapAngles: [0], tags: [] },
    { id: "door", role: "opening", snapAngles: [0], tags: [], socketCapable: true },
  ],
};
const TEMPLATE: ReachTemplate = {
  criticalPath: ["hub", "seg1", "seg2", "seg3", "capstone", "terminal"],
  nodes: {
    hub: { id: "hub", role: "hub", slots: { min: 3, max: 4 }, bootstrap: true },
    seg1: { id: "seg1", role: "segment", slots: { min: 1, max: 2 } },
    seg2: { id: "seg2", role: "segment", slots: { min: 1, max: 2 } },
    seg3: { id: "seg3", role: "segment", slots: { min: 1, max: 2 } },
    capstone: { id: "capstone", role: "capstone", slots: { min: 1, max: 1 } },
    terminal: { id: "terminal", role: "terminal", slots: { min: 1, max: 1 } },
  },
  branches: [{ anchor: "any-segment", role: "vault", slots: { min: 1, max: 2 }, entrance: "single" }],
  gating: { lockFraction: 0.5, compoundChance: 0.25, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true },
};

function ctx(): ComposeContext {
  return {
    registry: defineRegistry({
      grid: { areaCellSize: 16, roomCellSize: 2, snap: "ps2" },
      geometryKit: KIT,
      items: { catalog: [{ id: "a", class: "progression", grants: "grapple" }, { id: "b", class: "progression", grants: "reveal" }], startCaps: [] },
      locks: { g: (r) => r.have("grapple") },
      styles: { s: { id: "s" } },
    }),
    seed: "orch",
  };
}

describe("orchestration", () => {
  it("composeWorldAsync is byte-identical to composeWorld (schedules the same sync core)", async () => {
    const sync = composeWorld(ctx(), { reachCount: 3, template: TEMPLATE, carryCaps: true });
    const async = await composeWorldAsync(ctx(), { reachCount: 3, template: TEMPLATE, carryCaps: true });
    expect(JSON.stringify(async.descriptor)).toBe(JSON.stringify(sync.descriptor));
  });

  it("reports progress per reach and streams each", async () => {
    const seen: number[] = [];
    let reachCount = 0;
    await composeWorldAsync(ctx(), { reachCount: 3, template: TEMPLATE }, { onProgress: (p) => seen.push(p.index), onReach: () => reachCount++ });
    expect(seen).toEqual([1, 2, 3]);
    expect(reachCount).toBe(3);
  });

  it("honours cancellation", async () => {
    const token = new CancellationToken();
    token.cancel("test");
    await expect(composeReachAsync(ctx(), { template: TEMPLATE }, { signal: token })).rejects.toBeInstanceOf(GenCancelled);
  });

  it("GenerationHorizon lazily composes + caches reaches", () => {
    const h = new GenerationHorizon(ctx(), { reachCount: 5, template: TEMPLATE, carryCaps: true });
    expect(h.has(2)).toBe(false);
    const r2 = h.reach(2);
    expect(h.has(2)).toBe(true);
    expect(h.reach(2)).toBe(r2); // cached identity
    const pre = h.prefetch(3, 1);
    expect(pre.length).toBe(2);
    expect(h.has(4)).toBe(true);
    h.evictOutside(3, 4);
    expect(h.has(2)).toBe(false);
    expect(h.has(3)).toBe(true);
  });
});
