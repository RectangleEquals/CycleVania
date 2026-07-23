import { describe, it, expect } from "vitest";
import { flag } from "../logic/index.js";
import { GenError } from "../errors.js";
import { MemorySink } from "../diagnostics.js";
import type { CapabilityDef, GadgetDef } from "../capability/index.js";
import { collectathon, capabilityLock, type PuzzleDef } from "../puzzle/index.js";
import type { ReachTemplate, ReachTemplatePool } from "../template/index.js";
import { defineRegistry, type RegistryInput } from "./define-registry.js";

const caps: CapabilityDef[] = [
  { id: "jump", held: "granted", facets: [{ kind: "magnitude", bucket: "traversal.zUp", evaluate: () => 2 }], powerWeight: () => 0.5 },
  { id: "grapple", held: "granted", facets: [{ kind: "tag", tag: "grapple-point" }], powerWeight: () => 0.4 },
  { id: "fragment", held: "granted", facets: [], powerWeight: () => 0.2 },
];
const gadgets: GadgetDef[] = [
  { id: "boots", grants: ["jump"] },
  { id: "hook", grants: ["grapple"] },
  { id: "frag0", grants: ["fragment"] },
  { id: "frag1", grants: ["fragment"] },
  { id: "frag2", grants: ["fragment"] },
  { id: "frag3", grants: ["fragment"] },
];
const puzzles: PuzzleDef[] = [collectathon("shrine", "fragment", 4, "impact"), capabilityLock("chasm", "grapple", "across")];
const template: ReachTemplate = {
  id: "t",
  criticalPath: ["hub", "seg", "gate", "cap", "term"],
  nodes: {
    hub: { role: "hub", slots: { min: 10, max: 10 } },
    seg: { role: "segment", slots: { min: 2, max: 2 } },
    gate: { role: "gate", slots: { min: 1, max: 1 } },
    cap: { role: "capstone", slots: { min: 1, max: 1 } },
    term: { role: "terminal", slots: { min: 1, max: 1 } },
  },
  branches: [],
  gating: { lockFraction: 0.5, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true, density: 0 },
};
const pool: ReachTemplatePool = { poolAt: () => [{ template, weight: 1 }] };
const base: RegistryInput = { gadgets: { capabilities: caps, gadgets }, puzzles, templatePool: pool };

const codeOf = (fn: () => unknown): string => {
  try {
    fn();
  } catch (e) {
    return e instanceof GenError ? e.code : "not-a-generror";
  }
  return "no-throw";
};

describe("defineRegistry validation", () => {
  it("accepts a valid registry", () => {
    expect(() => defineRegistry(base)).not.toThrow();
  });

  it("rejects duplicate ids", () => {
    expect(codeOf(() => defineRegistry({ ...base, gadgets: { capabilities: [...caps, caps[0] as CapabilityDef], gadgets } }))).toBe("registry.duplicate-id");
  });

  it("rejects a gadget granting an unknown capability", () => {
    expect(codeOf(() => defineRegistry({ ...base, gadgets: { capabilities: caps, gadgets: [...gadgets, { id: "x", grants: ["ghost"] }] } }))).toBe("registry.unknown-grant");
  });

  it("rejects derivation from an unknown capability and a derivation cycle", () => {
    const badDerived: CapabilityDef = { id: "combo", held: { derivedFrom: ["missing"] }, facets: [], powerWeight: () => 0.5 };
    expect(codeOf(() => defineRegistry({ ...base, gadgets: { capabilities: [...caps, badDerived], gadgets } }))).toBe("registry.unknown-derived");
    const a: CapabilityDef = { id: "a", held: { derivedFrom: ["b"] }, facets: [], powerWeight: () => 0.5 };
    const b: CapabilityDef = { id: "b", held: { derivedFrom: ["a"] }, facets: [], powerWeight: () => 0.5 };
    expect(codeOf(() => defineRegistry({ ...base, gadgets: { capabilities: [...caps, a, b], gadgets } }))).toBe("registry.derived-cycle");
  });

  it("rejects a required puzzle gating on a volatile flag", () => {
    const bad: PuzzleDef = { id: "vol", scope: "room", class: "required", condition: flag("timed", { volatile: true }), outcome: { kind: "set-flag-only" }, powerWeight: () => 0.5 };
    expect(codeOf(() => defineRegistry({ ...base, puzzles: [...puzzles, bad] }))).toBe("registry.volatile-required");
  });

  it("rejects an unknown recipe reference and an unknown condition capability", () => {
    const badRecipe: PuzzleDef = { id: "r", scope: "room", class: "required", condition: flag("f"), outcome: { kind: "set-flag-only" }, powerWeight: () => 0.5, spatialRecipe: "nope" };
    expect(codeOf(() => defineRegistry({ ...base, puzzles: [...puzzles, badRecipe] }))).toBe("registry.unknown-recipe");
    const badCap = capabilityLock("bc", "ghost-cap", "x");
    expect(codeOf(() => defineRegistry({ ...base, puzzles: [...puzzles, badCap] }))).toBe("registry.unknown-condition-cap");
  });

  it("rejects an invalid economy and a hubless template", () => {
    expect(codeOf(() => defineRegistry({ ...base, gadgetEconomy: { min: 0, max: 2 } }))).toBe("registry.bad-economy");
    const noHub: ReachTemplate = { ...template, nodes: { ...template.nodes, hub: { role: "segment", slots: { min: 10, max: 10 } } } };
    expect(codeOf(() => defineRegistry({ ...base, templatePool: { poolAt: () => [{ template: noHub, weight: 1 }] } }))).toBe("registry.template-no-hub");
  });
});

describe("defineRegistry fingerprint", () => {
  it("is stable for identical input and changes with content/revision", () => {
    expect(defineRegistry(base).fingerprint).toBe(defineRegistry(base).fingerprint);
    const revd = caps.map((c) => (c.id === "jump" ? { ...c, revision: 7 } : c));
    expect(defineRegistry({ ...base, gadgets: { capabilities: revd, gadgets } }).fingerprint).not.toBe(defineRegistry(base).fingerprint);
    expect(defineRegistry({ ...base, gadgetEconomy: { min: 2, max: 5 } }).fingerprint).not.toBe(defineRegistry(base).fingerprint);
  });
});

describe("defineRegistry soft warnings", () => {
  it("warns about a never-referenced capability without throwing", () => {
    const sink = new MemorySink();
    const orphanCap: CapabilityDef = { id: "orphan", held: "granted", facets: [], powerWeight: () => 0.5 };
    defineRegistry({ ...base, gadgets: { capabilities: [...caps, orphanCap], gadgets }, diagnostics: { level: "warn", sink } });
    expect(sink.events.some((e) => e.code === "registry.unreachable-entry")).toBe(true);
  });
});
