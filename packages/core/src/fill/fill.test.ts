import { describe, it, expect } from "vitest";
import { have, count } from "../logic/index.js";
import { Rng } from "../math/rng.js";
import { assumedFill } from "./assumed-fill.js";
import { isSolvable, validateGraph } from "../graph/solvable.js";
import type { ProgressionItem, RegionGraph } from "../graph/region-graph.js";

// sanctum over-provisions bootstrap slots so fill can never corner.
function gatedGraph(): RegionGraph {
  return {
    start: "sanctum",
    regions: new Set(["sanctum", "gap", "ledge", "deep"]),
    edges: [
      { from: "sanctum", to: "gap", rule: have("tether") },
      { from: "sanctum", to: "ledge", rule: have("impeller") },
      { from: "gap", to: "deep", rule: have("impeller") },
      { from: "ledge", to: "deep", rule: have("tether") },
    ],
    locations: new Map([
      ["boot0", "sanctum"],
      ["boot1", "sanctum"],
      ["boot2", "sanctum"],
      ["gapCache", "gap"],
      ["ledgeCache", "ledge"],
      ["deepReward", "deep"],
    ]),
  };
}
const ITEMS: ProgressionItem[] = [
  { id: "tether", grants: "tether" },
  { id: "impeller", grants: "impeller" },
];
const byId = new Map(ITEMS.map((i) => [i.id, i]));

describe("assumedFill", () => {
  it("produces a solvable placement across many seeds", () => {
    const g = gatedGraph();
    expect(validateGraph(g).ok).toBe(true);
    for (let s = 0; s < 500; s++) {
      const placement = assumedFill(g, ITEMS, [], new Rng(`seed-${s}`));
      expect(placement).not.toBeNull();
      expect(isSolvable(g, placement!, byId, [])).toBe(true);
    }
  });

  it("is deterministic for a given seed", () => {
    const g = gatedGraph();
    const a = assumedFill(g, ITEMS, [], new Rng("fixed"));
    const b = assumedFill(g, ITEMS, [], new Rng("fixed"));
    expect([...a!.entries()].sort()).toEqual([...b!.entries()].sort());
  });

  it("handles counted (multi-key) locks without softlocks", () => {
    const shards: ProgressionItem[] = [
      { id: "shard-a", grants: "shard" },
      { id: "shard-b", grants: "shard" },
      { id: "shard-c", grants: "shard" },
    ];
    const shardsById = new Map(shards.map((i) => [i.id, i]));
    const g: RegionGraph = {
      start: "sanctum",
      regions: new Set(["sanctum", "reliquary"]),
      edges: [{ from: "sanctum", to: "reliquary", rule: count("shard", 3) }],
      locations: new Map([
        ["boot0", "sanctum"],
        ["boot1", "sanctum"],
        ["boot2", "sanctum"],
        ["boot3", "sanctum"],
        ["relic", "reliquary"],
      ]),
    };
    for (let s = 0; s < 200; s++) {
      const placement = assumedFill(g, shards, [], new Rng(`c-${s}`));
      expect(placement).not.toBeNull();
      expect(isSolvable(g, placement!, shardsById, [])).toBe(true);
      // the 3 shards can't all be gated behind the count(3) reliquary
      const inReliquary = [...placement!.entries()].filter(([loc]) => loc === "relic");
      expect(inReliquary.length).toBeLessThanOrEqual(1);
    }
  });
});
