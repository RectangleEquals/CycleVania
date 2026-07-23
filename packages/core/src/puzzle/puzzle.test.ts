import { describe, it, expect } from "vitest";
import { evalRule, heldOf } from "../logic/index.js";
import {
  capabilityLock,
  switchLock,
  panelArray,
  arenaLockdown,
  collectathon,
  bossGate,
  instantiate,
  outcomeGrantsCapability,
  outcomeOpensEdge,
  puzzleEntry,
} from "./index.js";

describe("lock vocabulary", () => {
  it("produces valid, evaluable defs for each taxonomy pattern", () => {
    expect(evalRule(capabilityLock("a", "grapple", "x").condition, heldOf(["grapple"]))).toBe(true);
    expect(evalRule(switchLock("b", "sw", "x").condition, heldOf([], ["sw"]))).toBe(true);
    expect(evalRule(panelArray("c", ["p1", "p2"], "x").condition, heldOf([], ["p1", "p2"]))).toBe(true);
    expect(evalRule(panelArray("c", ["p1", "p2"], "x").condition, heldOf([], ["p1"]))).toBe(false);
    expect(evalRule(arenaLockdown("d", "cleared", "x").condition, heldOf([], ["cleared"]))).toBe(true);
    expect(evalRule(collectathon("e", "frag", 3, "x").condition, heldOf([]).add("frag").add("frag").add("frag"))).toBe(true);
    expect(evalRule(bossGate("f", "boss-dead", "x").condition, heldOf([], ["boss-dead"]))).toBe(true);
  });

  it("collectathon carries a scope:world count condition + guarantee", () => {
    const p = collectathon("shrine", "artifact", 12, "final", { guaranteeWithin: 6 });
    expect(p.scope).toBe("world");
    expect(p.guarantee).toEqual({ withinReachLevels: 6 });
  });
});

describe("instantiate + outcomes", () => {
  it("shares the def's condition Rule BY REFERENCE", () => {
    const def = capabilityLock("a", "grapple", "x");
    const inst = instantiate(def, "a#r0");
    expect(inst.condition).toBe(def.condition); // === identity (the coupling invariant)
    expect(inst.instanceId).toBe("a#r0");
  });

  it("exposes outcome introspection", () => {
    expect(outcomeOpensEdge(capabilityLock("a", "grapple", "dest").outcome)?.to).toBe("dest");
    expect(outcomeGrantsCapability({ kind: "grant-capability", capability: "varia" })).toBe("varia");
    expect(outcomeGrantsCapability({ kind: "set-flag-only" })).toBeUndefined();
  });

  it("maps a def to a schedulable entry (id + powerWeight + guarantee)", () => {
    const entry = puzzleEntry(collectathon("s", "f", 4, "x", { guaranteeWithin: 5 }));
    expect(entry.id).toBe("s");
    expect(entry.guarantee).toEqual({ withinReachLevels: 5 });
  });
});
