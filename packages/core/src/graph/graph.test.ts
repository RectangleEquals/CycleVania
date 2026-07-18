import { describe, it, expect } from "vitest";
import { have } from "../logic/index.js";
import { heldOf } from "../logic/held.js";
import { reachableRegions } from "./reachability.js";
import { computeSpheres } from "./spheres.js";
import { isSolvable, hasCycle, validateGraph } from "./solvable.js";
import type { ProgressionItem, RegionGraph } from "./region-graph.js";

// sanctum → gap(tether) → deep(impeller); sanctum → ledge(impeller) → deep(tether)
function diamond(): RegionGraph {
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

describe("region graph", () => {
  it("reachability respects gates (fixed-point BFS)", () => {
    const g = diamond();
    expect(reachableRegions(g, heldOf([])).size).toBe(1); // sanctum only
    expect([...reachableRegions(g, heldOf(["tether"]))].sort()).toEqual(["gap", "sanctum"]);
    expect(reachableRegions(g, heldOf(["tether", "impeller"])).size).toBe(4);
  });

  it("validateGraph passes when everything is reachable fully-equipped", () => {
    expect(validateGraph(diamond()).ok).toBe(true);
  });

  it("validateGraph flags a stranded region", () => {
    const g = diamond();
    const broken: RegionGraph = {
      ...g,
      regions: new Set([...g.regions, "island"]), // no edge into it
    };
    const v = validateGraph(broken);
    expect(v.ok).toBe(false);
    expect(v.stranded).toContain("island");
  });

  it("hasCycle: diamond is acyclic; a back-edge makes it cyclic", () => {
    const g = diamond();
    expect(hasCycle(g)).toBe(false);
    const looped: RegionGraph = { ...g, edges: [...g.edges, { from: "deep", to: "sanctum", rule: have("tether") }] };
    expect(hasCycle(looped)).toBe(true);
  });

  it("computeSpheres + isSolvable across a valid placement", () => {
    const g = diamond();
    // tether in a boot slot, impeller in the gap (reachable once tether held)
    const placement = new Map([
      ["boot0", "tether"],
      ["gapCache", "impeller"],
    ]);
    const res = computeSpheres(g, placement, byId, []);
    expect(res.reachedAll).toBe(true);
    expect(res.held.has("tether")).toBe(true);
    expect(res.held.has("impeller")).toBe(true);
    expect(isSolvable(g, placement, byId, [])).toBe(true);
  });

  it("isSolvable is false when an item is locked behind itself", () => {
    const g = diamond();
    // impeller behind the ledge which requires impeller → unreachable
    const bad = new Map([["ledgeCache", "impeller"]]);
    expect(isSolvable(g, bad, byId, [])).toBe(false);
  });
});
