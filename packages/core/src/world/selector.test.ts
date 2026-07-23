import { describe, it, expect } from "vitest";
import { Rng } from "../math/index.js";
import { heldOf } from "../logic/index.js";
import type { CapabilityDef, GadgetDef } from "../capability/index.js";
import { collectathon, capabilityLock, type PuzzleDef } from "../puzzle/index.js";
import { defineRegistry, type RegistryInput } from "../registries/index.js";
import type { ReachTemplate, ReachTemplatePool } from "../template/index.js";
import { createRegistrySelector, worldFromRegistry } from "./content-selector.js";
import type { SelectionContext } from "./world-composer.js";

const facetCap = (id: string): CapabilityDef => ({ id, held: "granted", facets: [{ kind: "tag", tag: `${id}-tag` }], powerWeight: () => 0.5 });
const keyCap = (id: string): CapabilityDef => ({ id, held: "granted", facets: [], powerWeight: () => 0.2 });

const template = (hubSlots: number): ReachTemplate => ({
  id: "t",
  criticalPath: ["hub", "seg", "gate", "cap", "term"],
  nodes: {
    hub: { role: "hub", slots: { min: hubSlots, max: hubSlots } },
    seg: { role: "segment", slots: { min: 2, max: 2 } },
    gate: { role: "gate", slots: { min: 1, max: 1 } },
    cap: { role: "capstone", slots: { min: 1, max: 1 } },
    term: { role: "terminal", slots: { min: 1, max: 1 } },
  },
  branches: [],
  gating: { lockFraction: 0.5, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true, density: 0 },
});
const poolOf = (t: ReachTemplate): ReachTemplatePool => ({ poolAt: () => [{ template: t, weight: 1 }] });

const makeCtx = (o: Partial<SelectionContext> = {}): SelectionContext => ({
  reachIndex: 0,
  reachLevel: 0,
  finalCeiling: 100,
  isFinalReach: false,
  startHeld: heldOf([]),
  rng: new Rng("ctx"),
  chosenModifiers: [],
  placedLevels: new Map(),
  firstEligibleLevel: new Map(),
  gadgetPlan: new Map(),
  puzzlePlan: new Map(),
  ...o,
});

describe("content selector — teach → test → combine", () => {
  const caps = [facetCap("c0"), facetCap("c1"), facetCap("c2")];
  const gadgets: GadgetDef[] = [{ id: "g0", grants: ["c0"] }, { id: "g1", grants: ["c1"] }, { id: "g2", grants: ["c2"] }];

  it("emits a simple teach gate first, all-simple when combineChance is 0", () => {
    const reg = defineRegistry({ gadgets: { capabilities: caps, gadgets }, gadgetEconomy: { min: 3, max: 3 }, lockPacing: { teachTestCombine: true, combineChance: 0 }, templatePool: poolOf(template(6)) });
    const res = createRegistrySelector(reg).select(makeCtx());
    expect(res.gateRules.length).toBe(3);
    expect(res.gateRules.every((r) => r.k === "have")).toBe(true); // all simple
    expect(res.gateRules[0]?.k).toBe("have"); // teach first
  });

  it("composes later gates with an earlier requirement when combineChance is 1", () => {
    const reg = defineRegistry({ gadgets: { capabilities: caps, gadgets }, gadgetEconomy: { min: 3, max: 3 }, lockPacing: { teachTestCombine: true, combineChance: 1 }, templatePool: poolOf(template(6)) });
    const res = createRegistrySelector(reg).select(makeCtx());
    expect(res.gateRules[0]?.k).toBe("have"); // teach (k=0) never composed
    expect(res.gateRules.slice(1).some((r) => r.k === "and")).toBe(true); // later gates combine
  });
});

describe("content selector — pool independence", () => {
  const caps = [facetCap("c0"), facetCap("c1")];
  const gadgets: GadgetDef[] = [{ id: "g0", grants: ["c0"] }, { id: "g1", grants: ["c1"] }];
  const base = (puzzles: PuzzleDef[]): RegistryInput => ({ gadgets: { capabilities: caps, gadgets }, puzzles, templatePool: poolOf(template(8)) });

  it("changing the puzzle catalog never perturbs gadget scheduling for the same seed/request", () => {
    const wA = worldFromRegistry(defineRegistry(base([])), "poolindep");
    const wB = worldFromRegistry(defineRegistry(base([capabilityLock("chasm", "c0", "x")])), "poolindep");
    const a = wA.requestReach({ reachIndex: 0, chosenModifiers: [] });
    const b = wB.requestReach({ reachIndex: 0, chosenModifiers: [] });
    const gadgetItems = (r: typeof a): string[] => r.items.filter((i) => i.class === "progression").map((i) => i.id).sort();
    expect(gadgetItems(b)).toEqual(gadgetItems(a));
  });
});

describe("content selector — registry-driven world", () => {
  const caps = [facetCap("jump"), facetCap("grapple")];
  const gadgets: GadgetDef[] = [{ id: "boots", grants: ["jump"] }, { id: "hook", grants: ["grapple"] }];

  it("realizes a solvable Reach across several requests", () => {
    const reg = defineRegistry({ gadgets: { capabilities: caps, gadgets }, gadgetEconomy: { min: 1, max: 2 }, templatePool: poolOf(template(8)) });
    const w = worldFromRegistry(reg, "rdw");
    // assumedFill asserts solvability internally; a throw would fail the test.
    expect(() => {
      w.requestReach({ reachIndex: 0, chosenModifiers: [] });
      w.requestReach({ reachIndex: 1, fromReachIndex: 0, chosenModifiers: [] });
    }).not.toThrow();
  });
});

describe("content selector — world-scope collectathon", () => {
  it("places every fragment and a count-lock shrine in a single-Reach World, solvably", () => {
    const caps = [keyCap("fragment"), facetCap("beam")];
    const gadgets: GadgetDef[] = [
      { id: "beam-gun", grants: ["beam"] },
      { id: "frag0", grants: ["fragment"] },
      { id: "frag1", grants: ["fragment"] },
      { id: "frag2", grants: ["fragment"] },
      { id: "frag3", grants: ["fragment"] },
    ];
    const puzzles: PuzzleDef[] = [collectathon("shrine", "fragment", 4, "impact")];
    const reg = defineRegistry({
      gadgets: { capabilities: caps, gadgets },
      puzzles,
      lengthPolicy: { min: 1, max: 1 }, // L=1 → Reach 0 is final → sweep places everything
      templatePool: poolOf(template(12)),
    });
    const w = worldFromRegistry(reg, "collect");
    const r = w.requestReach({ reachIndex: 0, chosenModifiers: [] }); // throws if unsolvable
    // all four fragments placed as progression items
    expect(r.items.filter((i) => i.grants?.includes("fragment")).length).toBe(4);
    // the count(fragment, 4) lock is bound to an edge
    expect(r.graph.edges.some((e) => e.rule.k === "count" && e.rule.cap === "fragment" && e.rule.n === 4)).toBe(true);
    // the shrine instance is carried for spatial realization
    expect(r.puzzleInstances.some((p) => p.defId === "shrine")).toBe(true);
  });
});
