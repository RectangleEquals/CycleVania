import { describe, it, expect } from "vitest";
import { have } from "../logic/index.js";
import { GenError } from "../errors.js";
import type { Item } from "../graph/index.js";
import type { ReachTemplate, ReachTemplatePool } from "../template/index.js";
import { createWorld, verbatimSelector, type ReachResult, type WorldConfig } from "./world-composer.js";
import type { ReachModifierDef, ReachModifierPolicy } from "./modifiers.js";
import type { ReachRequest } from "./reach-request.js";

const demoTemplate: ReachTemplate = {
  id: "w",
  criticalPath: ["hub", "s1", "s2", "gate1", "s3", "capstone", "terminal"],
  nodes: {
    hub: { role: "hub", slots: { min: 6, max: 6 } },
    s1: { role: "segment", slots: { min: 2, max: 2 } },
    s2: { role: "segment", slots: { min: 1, max: 2 } },
    gate1: { role: "gate", slots: { min: 1, max: 1 } },
    s3: { role: "segment", slots: { min: 1, max: 2 } },
    capstone: { role: "capstone", slots: { min: 1, max: 1 } },
    terminal: { role: "terminal", slots: { min: 0, max: 1 } },
  },
  branches: [{ attachTo: "s1", role: "vault", entrance: "single", slots: { min: 1, max: 2 }, backEdgeChance: 0.3 }],
  gating: { lockFraction: 0.5, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true, density: 0.2 },
};
const pool: ReachTemplatePool = { poolAt: () => [{ template: demoTemplate, weight: 1 }] };
const items: Item[] = [
  { id: "iA", class: "progression", grants: ["capA"] },
  { id: "iB", class: "progression", grants: ["capB"] },
  { id: "iC", class: "progression", grants: ["capC"] },
  { id: "iD", class: "progression", grants: ["capD"] },
  { id: "f1", class: "filler" },
];
const gateRules = [have("capA"), have("capB"), have("capC")];

const m1: ReachModifierDef = { id: "m1", riskWeight: 0.2, rewardWeight: 0.2, minDepth: 0, dials: { complexity: { additive: 15 } } };
const m2: ReachModifierDef = { id: "m2", riskWeight: 0.8, rewardWeight: 0.8, minDepth: 0, dials: { complexity: { multiplier: 0.2 } } };
const mLate: ReachModifierDef = { id: "mLate", riskWeight: 0.9, rewardWeight: 0.9, minDepth: 5, dials: { complexity: { multiplier: 0.5 } } };
const catalog = [m1, m2, mLate];
const modifierPolicy: ReachModifierPolicy = {
  poolAt: (d) => catalog.filter((m) => m.minDepth <= d),
  requiredRange: () => ({ min: 0, max: 2 }),
};

const makeConfig = (seed: string): WorldConfig => ({
  seed,
  templatePool: pool,
  selector: verbatimSelector(items, gateRules),
  modifierPolicy,
  modifierCatalog: catalog,
});

const canon = (r: ReachResult): string =>
  JSON.stringify({
    meta: r.meta,
    placement: [...r.placement.entries()].sort(),
    regions: r.graph.regions,
    edges: r.graph.edges.map((e) => ({ from: e.from, to: e.to, rule: e.rule, oneWay: e.oneWay ?? false })),
    locations: r.graph.locations,
  });

describe("WorldComposer.requestReach", () => {
  it("realizes a solvable Reach and records exactly one request-log entry", () => {
    const w = createWorld(makeConfig("s"));
    const r0 = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
    expect(r0.meta.reachIndex).toBe(0);
    expect(w.realized.size).toBe(1);
    expect(w.requestLog).toHaveLength(1);
    expect(w.portals.some((p) => p.fromReach === 0)).toBe(true); // terminal auto-portal
  });

  it("is reproducible from (seed, requestLog)", () => {
    const wA = createWorld(makeConfig("repro"));
    const requests: ReachRequest[] = [
      { reachIndex: 0, chosenModifiers: [] },
      { reachIndex: 1, fromReachIndex: 0, chosenModifiers: ["m1"] },
      { reachIndex: 2, fromReachIndex: 1, chosenModifiers: ["m2"], gadgetEconomyOverride: { max: 4 } },
      { reachIndex: 3, fromReachIndex: 2, chosenModifiers: ["m1", "m2"] },
    ];
    const resultsA = requests.map((rq) => wA.requestReach(rq));

    const wB = createWorld(makeConfig("repro"));
    const resultsB = wA.requestLog.map((r) => wB.requestReach(r.request));

    for (let i = 0; i < resultsA.length; i++) expect(canon(resultsB[i] as ReachResult)).toBe(canon(resultsA[i] as ReachResult));
  });

  it("rejects illegal requests with a typed GenError", () => {
    const codeOf = (fn: () => unknown): string => {
      try {
        fn();
      } catch (e) {
        return e instanceof GenError ? e.code : "not-a-generror";
      }
      return "no-throw";
    };
    const w = createWorld(makeConfig("v"));
    w.requestReach({ reachIndex: 0, chosenModifiers: [] });
    expect(codeOf(() => w.requestReach({ reachIndex: 0, chosenModifiers: [] }))).toBe("request.duplicate");
    expect(codeOf(() => w.requestReach({ reachIndex: 1, chosenModifiers: [] }))).toBe("request.missing-origin");

    const w2 = createWorld(makeConfig("v2"));
    w2.requestReach({ reachIndex: 0, chosenModifiers: [] });
    expect(codeOf(() => w2.requestReach({ reachIndex: 2, fromReachIndex: 5, chosenModifiers: [] }))).toBe("request.unrealized-origin");
    expect(codeOf(() => w2.requestReach({ reachIndex: 1, fromReachIndex: 0, chosenModifiers: ["mLate"] }))).toBe("modifier.depth");
  });

  it("carries capabilities forward across Reaches", () => {
    const w = createWorld(makeConfig("carry"));
    const r0 = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
    const r1 = w.requestReach({ reachIndex: 1, fromReachIndex: 0, chosenModifiers: [] });
    // Reach 1 starts holding everything Reach 0 granted.
    expect(r0.meta.startHeld.caps).toEqual({});
    expect(Object.keys(r1.meta.startHeld.caps).sort()).toEqual(["capA", "capB", "capC", "capD"]);
  });
});

describe("WorldComposer length policy + portals", () => {
  it("draws L for a bounded World and marks the final Reach", () => {
    const w = createWorld({ ...makeConfig("len"), lengthPolicy: { min: 1, max: 1 } });
    expect(w.drawnLength).toBe(1);
    expect(w.previewReachEnvelope(0).isDeclaredFinalReach).toBe(true);
    expect(() => w.requestReach({ reachIndex: 1, fromReachIndex: 0, chosenModifiers: [] })).toThrow(/past-end|exceeds/);
  });

  it("is unbounded when the policy is omitted", () => {
    const w = createWorld(makeConfig("unb"));
    expect(w.drawnLength).toBeUndefined();
    expect(w.previewReachEnvelope(0).plannedCapabilities).toBeUndefined();
  });

  it("addPortal to an unrealized Reach throws", () => {
    const w = createWorld(makeConfig("port"));
    w.requestReach({ reachIndex: 0, chosenModifiers: [] });
    let code = "";
    try {
      w.addPortal({ fromReach: 0, toReach: 9, oneWay: false, fromSpaceHint: "a", toSpaceHint: "b" });
    } catch (e) {
      code = e instanceof GenError ? e.code : "";
    }
    expect(code).toBe("portal.unrealized");
  });
});
