import { describe, it, expect } from "vitest";
import {
  ALWAYS,
  have,
  count,
  flag,
  not,
  and,
  or,
  evalRule,
  ruleCaps,
  ruleFlags,
  missingCaps,
  usesVolatileFlag,
} from "./rule.js";
import { heldOf } from "./held.js";

describe("Rule algebra", () => {
  it("evaluates the primitives and connectives, including nesting", () => {
    const held = heldOf(["a", "b"], ["sw"]);
    expect(evalRule(ALWAYS, held)).toBe(true);
    expect(evalRule(have("a"), held)).toBe(true);
    expect(evalRule(have("z"), held)).toBe(false);
    expect(evalRule(flag("sw"), held)).toBe(true);
    expect(evalRule(flag("off"), held)).toBe(false);
    expect(evalRule(not(have("z")), held)).toBe(true);
    expect(evalRule(and(have("a"), have("b")), held)).toBe(true);
    expect(evalRule(and(have("a"), have("z")), held)).toBe(false);
    expect(evalRule(or(have("z"), have("a")), held)).toBe(true);
    // nested: (a AND (sw OR z)) AND NOT z
    expect(evalRule(and(have("a"), or(flag("sw"), have("z")), not(have("z"))), held)).toBe(true);
  });

  it("count is satisfied at exactly n copies", () => {
    const two = heldOf([]).add("k").add("k"); // 2 copies
    expect(evalRule(count("k", 3), two)).toBe(false);
    two.add("k"); // 3
    expect(evalRule(count("k", 3), two)).toBe(true);
  });

  it("ruleCaps and ruleFlags collect (deduped, in order)", () => {
    const r = and(have("a"), or(have("b"), have("a")), flag("f1"), not(flag("f2")));
    expect(ruleCaps(r)).toEqual(["a", "b"]);
    expect(ruleFlags(r)).toEqual(["f1", "f2"]);
    expect(ruleCaps(ALWAYS)).toEqual([]);
    expect(ruleFlags(count("k", 2))).toEqual([]);
  });

  it("missingCaps returns the FULL unmet cap set for AND, nothing when satisfied", () => {
    const held = heldOf(["a"]);
    expect(missingCaps(and(have("a"), have("b"), flag("f")), held)).toEqual(["b"]); // flags are not caps
    expect(missingCaps(or(have("a"), have("b")), held)).toEqual([]); // already satisfied
    expect(missingCaps(or(have("x"), have("y")), held)).toEqual(["x", "y"]); // both alternatives unmet
    expect(missingCaps(have("a"), held)).toEqual([]);
  });

  it("usesVolatileFlag detects a volatile flag through arbitrary nesting", () => {
    expect(usesVolatileFlag(flag("t", { volatile: true }))).toBe(true);
    expect(usesVolatileFlag(and(have("a"), or(flag("t", { volatile: true }), have("b"))))).toBe(true);
    expect(usesVolatileFlag(and(have("a"), flag("normal")))).toBe(false);
    expect(usesVolatileFlag(not(flag("t", { volatile: true })))).toBe(true);
  });
});
