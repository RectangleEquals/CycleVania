import { describe, it, expect } from "vitest";
import { ALWAYS, have, CapSet } from "../logic/index.js";
import type { CapabilityDef, GadgetDef } from "../capability/index.js";
import { defineRegistry } from "../registries/index.js";
import { worldFromRegistry } from "../world/index.js";
import type { ReachTemplate, ReachTemplatePool } from "../template/index.js";
import { buildSimWorld, type SimWorld } from "./world.js";
import { initSim, cloneState, type SimState } from "./state.js";
import { step } from "./reducer.js";
import { parseCommand } from "./parser.js";
import { autosolve } from "./autosolve.js";

const worldLit = (over: Partial<SimWorld> = {}): SimWorld => ({
  start: "A",
  terminal: "B",
  startHeld: new CapSet(),
  nodes: new Map([
    ["A", { id: "A", role: "hub", locations: [] }],
    ["B", { id: "B", role: "terminal", locations: [] }],
  ]),
  links: [{ from: "A", to: "B", rule: ALWAYS }],
  items: new Map(),
  flagSetters: new Map(),
  puzzles: [],
  spheres: [],
  ...over,
});

const canon = (s: SimState): string => JSON.stringify({ at: s.at, caps: s.held.capIds().sort(), collected: [...s.collected].sort(), visited: [...s.visited].sort() });

describe("reducer", () => {
  it("is pure — step twice from a cloned state gives identical results", () => {
    const w = worldLit();
    const s = initSim(w);
    const a = step(w, cloneState(s), { k: "move", to: "B" });
    const b = step(w, cloneState(s), { k: "move", to: "B" });
    expect(canon(a.state)).toBe(canon(b.state));
    expect(a.message).toBe(b.message);
  });

  it("blocks a gated move with the full unmet set, then opens after taking the key", () => {
    const w = worldLit({
      nodes: new Map([
        ["A", { id: "A", role: "hub", locations: [{ id: "L0", itemId: "keyitem" }] }],
        ["B", { id: "B", role: "terminal", locations: [] }],
      ]),
      links: [{ from: "A", to: "B", rule: have("key") }],
      items: new Map([["keyitem", { id: "keyitem", class: "progression", grants: ["key"] }]]),
    });
    let s = initSim(w);
    const blocked = step(w, s, { k: "move", to: "B" });
    expect(blocked.ok).toBe(false);
    expect(blocked.message).toContain("key");
    s = step(w, s, { k: "take" }).state;
    const opened = step(w, s, { k: "move", to: "B" });
    expect(opened.ok).toBe(true);
    expect(opened.state.at).toBe("B");
  });

  it("follows a one-way link forward only", () => {
    const w = worldLit({ links: [{ from: "A", to: "B", rule: ALWAYS, oneWay: true }] });
    const forward = step(w, initSim(w), { k: "move", to: "B" });
    expect(forward.ok).toBe(true);
    // from B, A is not reachable (no reverse)
    const back = step(w, forward.state, { k: "move", to: "A" });
    expect(back.ok).toBe(false);
  });

  it("clears a volatile flag when leaving the region that set it", () => {
    const w = worldLit();
    const s = initSim(w);
    s.held.addFlag("timed");
    s.volatileFlags.set("timed", "A");
    const after = step(w, s, { k: "move", to: "B" });
    expect(after.state.held.hasFlag("timed")).toBe(false);
    expect(after.state.volatileFlags.size).toBe(0);
  });

  it("see/why report open, blocked, and missing caps", () => {
    const w = worldLit({ links: [{ from: "A", to: "B", rule: have("wings") }] });
    const s = initSim(w);
    expect(step(w, s, { k: "why", to: "B" }).message).toContain("wings");
    expect(step(w, s, { k: "see" }).message).toContain("blocked");
  });

  it("parses slash commands", () => {
    expect(parseCommand("/move X")).toEqual({ k: "move", to: "X" });
    expect(parseCommand("/use skyhook")).toEqual({ k: "use", itemId: "skyhook" });
    expect(parseCommand("/take")).toEqual({ k: "take" });
    expect(parseCommand("/nonsense")).toBeUndefined();
  });
});

describe("autosolve", () => {
  const caps: CapabilityDef[] = [
    { id: "jump", held: "granted", facets: [{ kind: "tag", tag: "j" }], powerWeight: () => 0.5 },
    { id: "grapple", held: "granted", facets: [{ kind: "tag", tag: "g" }], powerWeight: () => 0.5 },
  ];
  const gadgets: GadgetDef[] = [{ id: "boots", grants: ["jump"] }, { id: "hook", grants: ["grapple"] }];
  const template: ReachTemplate = {
    id: "t",
    criticalPath: ["hub", "s1", "gate1", "capstone", "terminal"],
    nodes: {
      hub: { role: "hub", slots: { min: 6, max: 6 } },
      s1: { role: "segment", slots: { min: 1, max: 2 } },
      gate1: { role: "gate", slots: { min: 1, max: 1 } },
      capstone: { role: "capstone", slots: { min: 1, max: 1 } },
      terminal: { role: "terminal", slots: { min: 0, max: 1 } },
    },
    branches: [],
    gating: { lockFraction: 0.5, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
    loops: { guaranteeAtLeastOne: true, density: 0.2 },
  };
  const pool: ReachTemplatePool = { poolAt: () => [{ template, weight: 1 }] };
  const reg = defineRegistry({ gadgets: { capabilities: caps, gadgets }, gadgetEconomy: { min: 2, max: 2 }, templatePool: pool });

  it("reaches the terminal for 100 seeds (matching isSolvable)", () => {
    for (let seed = 0; seed < 100; seed++) {
      const w = worldFromRegistry(reg, `sim-${seed}`);
      const rr = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
      const result = autosolve(buildSimWorld(rr)); // throws if stuck
      expect(result.success).toBe(true);
      expect(Number.isFinite(result.movesBetweenRewards)).toBe(true);
    }
  });

  it("is unaffected by geometry being enabled", () => {
    const wOff = worldFromRegistry(reg, "geo-same");
    const wOn = worldFromRegistry(reg, "geo-same", { geometry: true });
    const off = autosolve(buildSimWorld(wOff.requestReach({ reachIndex: 0, chosenModifiers: [] })));
    const on = autosolve(buildSimWorld(wOn.requestReach({ reachIndex: 0, chosenModifiers: [] })));
    expect(off.success).toBe(true);
    expect(on.success).toBe(true);
    expect(off.state.collected.size).toBe(on.state.collected.size);
  });
});
