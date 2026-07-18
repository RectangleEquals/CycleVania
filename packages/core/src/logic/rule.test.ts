import { describe, it, expect } from "vitest";
import { ALWAYS, have, count, flag, not, and, or, evalRule, missingCaps, ruleCaps } from "./rule.js";
import { heldOf, CapSet } from "./held.js";

describe("rules", () => {
  it("evaluates the base connectives", () => {
    const held = heldOf(["tether", "impeller"]);
    expect(evalRule(ALWAYS, held)).toBe(true);
    expect(evalRule(have("tether"), held)).toBe(true);
    expect(evalRule(have("nope"), held)).toBe(false);
    expect(evalRule(and(have("tether"), have("impeller")), held)).toBe(true);
    expect(evalRule(and(have("tether"), have("nope")), held)).toBe(false);
    expect(evalRule(or(have("nope"), have("impeller")), held)).toBe(true);
    expect(evalRule(not(have("nope")), held)).toBe(true);
    expect(evalRule(not(have("tether")), held)).toBe(false);
  });

  it("counts multi-key locks", () => {
    const held = new CapSet().add("shard", 3);
    expect(evalRule(count("shard", 3), held)).toBe(true);
    expect(evalRule(count("shard", 4), held)).toBe(false);
  });

  it("reads event flags", () => {
    const held = heldOf([], ["west-lever"]);
    expect(evalRule(flag("west-lever"), held)).toBe(true);
    expect(evalRule(flag("east-lever"), held)).toBe(false);
  });

  it("missingCaps reports the unmet subset of a compound gate (never one primary)", () => {
    const gate = and(have("tether"), have("impeller"));
    expect([...missingCaps(gate, heldOf(["tether"]))]).toEqual(["impeller"]);
    expect(missingCaps(gate, heldOf(["tether", "impeller"])).size).toBe(0);
  });

  it("ruleCaps collects every referenced capability", () => {
    const caps = ruleCaps(or(have("a"), and(have("b"), count("c", 2))));
    expect([...caps].sort()).toEqual(["a", "b", "c"]);
  });
});
