import { describe, it, expect } from "vitest";
import { defineRegistry } from "../registries/registry.js";
import type { GeometryKit } from "../registries/geometry-kit.js";
import type { ReachTemplate } from "../template/template.js";
import { ruleCaps } from "../logic/index.js";
import { composeReach } from "../composers/reach-composer.js";
import { buildSimWorld, neighbors } from "./world.js";
import { autosolve } from "./autosolve.js";
import { initSim } from "./state.js";
import { step } from "./reducer.js";
import { parseCommand } from "./parser.js";

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
  branches: [{ anchor: "any-segment", role: "vault", slots: { min: 1, max: 2 }, entrance: "single" }],
  gating: { lockFraction: 0.5, compoundChance: 0.25, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true },
};

function reg() {
  return defineRegistry({
    grid: { areaCellSize: 16, roomCellSize: 2, snap: "ps2" },
    geometryKit: KIT,
    items: {
      catalog: [
        { id: "skyhook", class: "progression", grants: "grapple", name: "The Ardent Skyhook" },
        { id: "lantern", class: "progression", grants: "reveal", name: "The Wan Lantern" },
        { id: "censer", class: "progression", grants: "leap", name: "Censer of the Second Wind" },
      ],
      startCaps: [],
    },
    locks: { gate: (r) => r.have("grapple") },
    styles: { sunken: { id: "sunken" } },
  });
}

describe("simulator", () => {
  it("autosolve reaches the terminal for every seed (bot completes a Reach)", () => {
    const registry = reg();
    for (let s = 0; s < 200; s++) {
      const world = buildSimWorld(composeReach({ registry, seed: `sim-${s}` }, { template: TEMPLATE }), registry);
      const end = autosolve(world);
      expect(world.terminalAreaId).not.toBeNull();
      expect(end.visited.has(world.terminalAreaId as number)).toBe(true);
    }
  });

  it("parses REPL commands", () => {
    expect(parseCommand("/goto 7")).toEqual({ k: "goto", areaId: 7 });
    expect(parseCommand("use skyhook")).toEqual({ k: "use", itemId: "skyhook" });
    expect(parseCommand("/take")).toEqual({ k: "take" });
    expect(parseCommand("/give reveal")).toEqual({ k: "give", cap: "reveal" });
    expect(() => parseCommand("/frobnicate")).toThrow();
  });

  it("step: giving a gate's caps unblocks an adjacent move", () => {
    const registry = reg();
    const world = buildSimWorld(composeReach({ registry, seed: "walk" }, { template: TEMPLATE }), registry);
    let st = initSim(world);
    const opts = neighbors(world, st.areaId, st.held);
    expect(opts.length).toBeGreaterThan(0);
    const target = opts[0]!;
    for (const cap of ruleCaps(target.link.requires)) st = step(world, st, { k: "give", cap }).state;
    const res = step(world, st, { k: "goto", areaId: target.to });
    expect(res.ok).toBe(true);
    expect(res.state.areaId).toBe(target.to);
  });

  it("step is pure (does not mutate the input state)", () => {
    const registry = reg();
    const world = buildSimWorld(composeReach({ registry, seed: "pure" }, { template: TEMPLATE }), registry);
    const st = initSim(world);
    const before = st.areaId;
    step(world, st, { k: "give", cap: "grapple" });
    expect(st.held.has("grapple")).toBe(false); // original untouched
    expect(st.areaId).toBe(before);
  });
});
