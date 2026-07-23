import { describe, it, expect } from "vitest";
import { ALWAYS, have, count, flag, heldOf } from "../logic/index.js";
import { GenError } from "../errors.js";
import {
  reachableRegions,
  computeSpheres,
  isSolvable,
  validateGraph,
  type MissionGraph,
  type Region,
  type NodeRole,
  type Item,
  type Placement,
} from "./index.js";

const R = (id: string, role: NodeRole = "segment"): Region => ({ id, role });

describe("mission graph", () => {
  it("grows reachability exactly when a gating cap is added", () => {
    const g: MissionGraph = {
      regions: [R("start", "hub"), R("mid"), R("end")],
      edges: [
        { from: "start", to: "mid", rule: ALWAYS },
        { from: "mid", to: "end", rule: have("key") },
      ],
      flags: [],
      locations: [],
      start: "start",
    };
    expect(reachableRegions(g, heldOf([]))).toEqual(new Set(["start", "mid"]));
    expect(reachableRegions(g, heldOf(["key"]))).toEqual(new Set(["start", "mid", "end"]));
  });

  it("follows directed one-way edges forward only", () => {
    const forward: MissionGraph = {
      regions: [R("A", "hub"), R("B")],
      edges: [{ from: "A", to: "B", rule: ALWAYS, oneWay: true }],
      flags: [],
      locations: [],
      start: "A",
    };
    expect(reachableRegions(forward, heldOf([]))).toEqual(new Set(["A", "B"]));
    // Same edge, but starting from B: cannot traverse A->B in reverse.
    expect(reachableRegions({ ...forward, start: "B" }, heldOf([]))).toEqual(new Set(["B"]));
  });

  it("validateGraph throws GenError naming a structurally stranded region", () => {
    const g: MissionGraph = {
      regions: [R("start", "hub"), R("A"), R("orphan")],
      edges: [{ from: "start", to: "A", rule: ALWAYS }],
      flags: [],
      locations: [],
      start: "start",
    };
    let err: unknown;
    try {
      validateGraph(g, heldOf([]));
    } catch (e) {
      err = e;
    }
    expect(err).toBeInstanceOf(GenError);
    expect((err as GenError).code).toBe("graph.stranded-region");
    expect((err as GenError).message).toContain("orphan");
    expect((err as GenError).details?.["stranded"]).toEqual(["orphan"]);
  });

  it("validateGraph reports the blocking capabilities of an out-of-scope gate", () => {
    const g: MissionGraph = {
      regions: [R("start", "hub"), R("gated")],
      edges: [{ from: "start", to: "gated", rule: have("future-cap") }],
      flags: [],
      locations: [],
      start: "start",
    };
    let err: GenError | undefined;
    try {
      validateGraph(g, heldOf([])); // "fully equipped" this reach does NOT include future-cap
    } catch (e) {
      err = e as GenError;
    }
    expect(err?.message).toContain("gated");
    expect(err?.message).toContain("future-cap");
  });

  it("validateGraph throws on a flag with no valid setter", () => {
    const g: MissionGraph = {
      regions: [R("start", "hub"), R("B")],
      edges: [{ from: "start", to: "B", rule: flag("nobody-sets-this") }],
      flags: [],
      locations: [],
      start: "start",
    };
    expect(() => validateGraph(g, heldOf([], ["nobody-sets-this"]))).toThrow(GenError);
  });

  it("orders the copies of a counted key before the gate they open", () => {
    const g: MissionGraph = {
      regions: [R("start", "hub"), R("vault")],
      edges: [{ from: "start", to: "vault", rule: count("shard", 3) }],
      flags: [],
      locations: [
        { id: "k1", region: "start" },
        { id: "k2", region: "start" },
        { id: "k3", region: "start" },
        { id: "goal", region: "vault" },
      ],
      start: "start",
    };
    const items: Item[] = [
      { id: "shardkey", class: "progression", grants: ["shard"] },
      { id: "prize", class: "filler" },
    ];
    const placement: Placement = new Map([
      ["k1", "shardkey"],
      ["k2", "shardkey"],
      ["k3", "shardkey"],
      ["goal", "prize"],
    ]);
    const { spheres } = computeSpheres(g, heldOf([]), placement, items);
    expect(new Set(spheres[0])).toEqual(new Set(["k1", "k2", "k3"]));
    expect(spheres[1]).toEqual(["goal"]);
  });

  it("propagates a flag set in one region to gate an edge into another", () => {
    const g: MissionGraph = {
      regions: [R("A", "hub"), R("B"), R("C")],
      edges: [
        { from: "A", to: "B", rule: ALWAYS },
        { from: "B", to: "C", rule: flag("switch") },
      ],
      flags: [{ name: "switch", setBy: "sw" }],
      locations: [
        { id: "sw", region: "B" },
        { id: "goal", region: "C" },
      ],
      start: "A",
    };
    const { spheres, reachedAll } = computeSpheres(g, heldOf([]), new Map(), []);
    expect(spheres[0]).toEqual(["sw"]);
    expect(spheres[1]).toEqual(["goal"]);
    expect(reachedAll).toBe(true);
  });

  it("computeSpheres yields the exact expected partition", () => {
    const g: MissionGraph = {
      regions: [R("start", "hub"), R("R1"), R("R2")],
      edges: [
        { from: "start", to: "R1", rule: have("capA") },
        { from: "R1", to: "R2", rule: have("capB") },
      ],
      flags: [],
      locations: [
        { id: "L0", region: "start" },
        { id: "L1", region: "R1" },
        { id: "L2", region: "R2" },
      ],
      start: "start",
    };
    const items: Item[] = [
      { id: "iA", class: "progression", grants: ["capA"] },
      { id: "iB", class: "progression", grants: ["capB"] },
      { id: "iC", class: "filler" },
    ];
    const placement: Placement = new Map([
      ["L0", "iA"],
      ["L1", "iB"],
      ["L2", "iC"],
    ]);
    expect(computeSpheres(g, heldOf([]), placement, items).spheres).toEqual([["L0"], ["L1"], ["L2"]]);
  });

  it("isSolvable is false when a progression item hides behind its own grant, true when fixed", () => {
    const g: MissionGraph = {
      regions: [R("start", "hub"), R("R")],
      edges: [{ from: "start", to: "R", rule: have("capX") }],
      flags: [],
      locations: [
        { id: "locStart", region: "start" },
        { id: "locR", region: "R" },
      ],
      start: "start",
    };
    const items: Item[] = [{ id: "itemX", class: "progression", grants: ["capX"] }];
    expect(isSolvable(g, heldOf([]), items, new Map([["locR", "itemX"]]))).toBe(false);
    expect(isSolvable(g, heldOf([]), items, new Map([["locStart", "itemX"]]))).toBe(true);
  });

  it("validateGraph passes for a well-formed graph when fully equipped", () => {
    const g: MissionGraph = {
      regions: [R("start", "hub"), R("R1"), R("R2")],
      edges: [
        { from: "start", to: "R1", rule: have("capA") },
        { from: "R1", to: "R2", rule: have("capB") },
      ],
      flags: [],
      locations: [],
      start: "start",
    };
    expect(() => validateGraph(g, heldOf(["capA", "capB"]))).not.toThrow();
  });
});
