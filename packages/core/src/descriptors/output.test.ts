import { describe, it, expect } from "vitest";
import type { CapabilityDef, GadgetDef } from "../capability/index.js";
import { defineRegistry, type RegistryInput } from "../registries/index.js";
import { worldFromRegistry } from "../world/index.js";
import type { ReachModifierDef, ReachModifierPolicy } from "../world/index.js";
import type { ReachTemplate, ReachTemplatePool } from "../template/index.js";
import { assembleWorld, assembleReach } from "./assemble.js";
import { stableStringify, toTypedKit } from "./serialize.js";
import { GENERATION_VERSION } from "./version.js";

const caps: CapabilityDef[] = [
  { id: "jump", held: "granted", facets: [{ kind: "tag", tag: "j" }], powerWeight: () => 0.5 },
  { id: "grapple", held: "granted", facets: [{ kind: "tag", tag: "g" }], powerWeight: () => 0.5 },
];
const gadgets: GadgetDef[] = [{ id: "boots", grants: ["jump"] }, { id: "hook", grants: ["grapple"] }];
const template = (path: string[]): ReachTemplate => ({
  id: "t",
  criticalPath: path,
  nodes: Object.fromEntries(path.map((id, i) => [id, { role: i === 0 ? "hub" : i === path.length - 1 ? "terminal" : id.startsWith("gate") ? "gate" : "segment", slots: { min: i === 0 ? 6 : 1, max: i === 0 ? 6 : 1 } }])) as ReachTemplate["nodes"],
  branches: [],
  gating: { lockFraction: path.some((p) => p.startsWith("gate")) ? 1 : 0, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: false, density: 0 },
});
const pool = (path: string[]): ReachTemplatePool => ({ poolAt: () => [{ template: template(path), weight: 1 }] });

const m1: ReachModifierDef = { id: "m1", riskWeight: 0.2, rewardWeight: 0.2, minDepth: 0, dials: { complexity: { additive: 10 } } };
const modifierPolicy: ReachModifierPolicy = { poolAt: () => [m1], requiredRange: () => ({ min: 0, max: 2 }) };

const makeReg = (over: Partial<RegistryInput> = {}): RegistryInput => ({
  gadgets: { capabilities: caps, gadgets },
  gadgetEconomy: { min: 1, max: 2 },
  complexity: { BaseCeiling: 40, K_MUL: 0.4, K_ADD: 3, TIER_SIZE: 3, JITTER_FRAC: 0.08, LOOKBEHIND_PULL: 0.35, MIN_CEILING: 30, HARD_MAX: 200, ABSOLUTE_HARD_MAX: 300 },
  templatePool: pool(["hub", "term"]),
  modifierCatalog: [m1],
  modifierPolicy,
  ...over,
});

describe("whole-output determinism (flagship)", () => {
  const build = (geometry: boolean): string => {
    const w = worldFromRegistry(defineRegistry(makeReg({ lengthPolicy: { min: 3, max: 3 } })), "flagship", { geometry });
    w.requestReach({ reachIndex: 0, chosenModifiers: [] });
    w.requestReach({ reachIndex: 1, fromReachIndex: 0, chosenModifiers: ["m1"], gadgetEconomyOverride: { max: 2 } });
    return stableStringify(assembleWorld(w));
  };

  it("is byte-identical across fresh composers with geometry ON", () => {
    expect(build(true)).toBe(build(true));
  });
  it("is byte-identical across fresh composers with geometry OFF", () => {
    expect(build(false)).toBe(build(false));
  });
});

describe("additivity", () => {
  it("geometry is additive — abstract facts are identical whether it ran or not", () => {
    const reg = defineRegistry(makeReg());
    const wOff = worldFromRegistry(reg, "add");
    const wOn = worldFromRegistry(reg, "add", { geometry: true });
    const rOff = assembleReach(wOff.requestReach({ reachIndex: 0, chosenModifiers: [] }));
    const rOn = assembleReach(wOn.requestReach({ reachIndex: 0, chosenModifiers: [] }));
    // abstract parts identical
    expect(stableStringify({ meta: rOff.meta, graph: rOff.graph, placement: rOff.placement })).toBe(stableStringify({ meta: rOn.meta, graph: rOn.graph, placement: rOn.placement }));
    // geometry only on the ON run
    expect(rOff.areas[0]?.kit).toBeUndefined();
    expect(rOn.areas[0]?.kit).toBeDefined();
  });
});

describe("round-trip + typed kit", () => {
  it("re-stringifies byte-identically and converts buffers", () => {
    const w = worldFromRegistry(defineRegistry(makeReg()), "rt", { geometry: true });
    w.requestReach({ reachIndex: 0, chosenModifiers: [] });
    const s = stableStringify(assembleWorld(w));
    expect(stableStringify(JSON.parse(s))).toBe(s);
    const kit = assembleReach([...w.realized.values()][0]!).areas.find((a) => a.kit)?.kit;
    if (kit && kit.pieces[0]) {
      const typed = toTypedKit(kit);
      expect(typed.pieces[0]!.positions).toBeInstanceOf(Float32Array);
      // Float32 reduces precision by design; values stay close to the source doubles.
      const src = kit.pieces[0].positions;
      Array.from(typed.pieces[0]!.positions).forEach((v, i) => expect(v).toBeCloseTo(src[i]!, 4));
    }
  });
});

describe("meta completeness + sparseness", () => {
  it("carries version + fingerprint, and the fingerprint changes with a registry edit", () => {
    const w = worldFromRegistry(defineRegistry(makeReg()), "meta");
    const d = assembleWorld(w);
    expect(d.meta.generationVersion).toBe(GENERATION_VERSION);
    expect(d.meta.registryFingerprint.startsWith("fp_")).toBe(true);
    const other = defineRegistry(makeReg({ gadgetEconomy: { min: 2, max: 4 } }));
    expect(other.fingerprint).not.toBe(defineRegistry(makeReg()).fingerprint);
  });

  it("only realized Reaches appear (sparse)", () => {
    const w = worldFromRegistry(defineRegistry(makeReg({ lengthPolicy: { min: 5, max: 5 } })), "sparse");
    w.requestReach({ reachIndex: 0, chosenModifiers: [] });
    w.requestReach({ reachIndex: 2, fromReachIndex: 0, chosenModifiers: [] }); // skip slot 1 (branching)
    const d = assembleWorld(w);
    expect(d.reaches.map((r) => r.meta.reachIndex)).toEqual([0, 2]);
  });
});

describe("rule identity survives assembly", () => {
  it("a gated inter-area connector's serialized gate deep-equals the L1 edge's rule", () => {
    const w = worldFromRegistry(defineRegistry(makeReg({ templatePool: pool(["hub", "gate1", "term"]) })), "gate");
    const rr = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
    const d = assembleReach(rr);
    const gatedConn = d.interAreaConnectors.find((c) => c.gate);
    const gatedEdge = d.graph.edges.find((e) => e.rule.k !== "always");
    expect(gatedConn).toBeDefined();
    expect(gatedEdge).toBeDefined();
    expect(stableStringify(gatedConn!.gate)).toBe(stableStringify(gatedEdge!.rule));
  });
});
