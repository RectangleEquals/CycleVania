import { describe, it, expect } from "vitest";
import { Rng } from "../math/index.js";
import { ALWAYS, have, heldOf } from "../logic/index.js";
import { GenError } from "../errors.js";
import { validateGraph, type MissionGraph } from "../graph/index.js";
import { interpretTemplate } from "./interpret.js";
import { drawTemplate } from "./template-pool.js";
import type { ReachTemplate, ReachTemplatePool } from "./reach-template.js";

const findEdge = (g: MissionGraph, from: string, to: string) => g.edges.find((e) => e.from === from && e.to === to);

describe("template interpretation — gate binding", () => {
  const t: ReachTemplate = {
    id: "gates",
    criticalPath: ["hub", "seg", "gateNode", "seg2", "term"],
    nodes: {
      hub: { role: "hub", slots: { min: 5, max: 5 } },
      seg: { role: "segment", slots: { min: 1, max: 1 } },
      gateNode: { role: "gate", slots: { min: 1, max: 1 } },
      seg2: { role: "segment", slots: { min: 1, max: 1 } },
      term: { role: "terminal", slots: { min: 1, max: 1 } },
    },
    branches: [{ attachTo: "seg", role: "vault", entrance: "single", slots: { min: 1, max: 1 }, backEdgeChance: 0 }],
    gating: { lockFraction: 1, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
    loops: { guaranteeAtLeastOne: false, density: 0 },
  };

  it("binds gateRules verbatim in priority order (gate entrance, vault entrance, locked spine)", () => {
    const gateRules = [have("g1"), have("g2"), have("g3")];
    const g = interpretTemplate(t, { gateRules, progressionCount: 2 }, {}, new Rng("gb").fork("i"));
    // priority: gate-node entrance, then vault entrance, then the remaining locked spine edge
    expect(findEdge(g, "seg", "gateNode")?.rule).toBe(gateRules[0]);
    const vaultEdge = g.edges.find((e) => e.from === "seg" && e.to.startsWith("seg~vault"));
    expect(vaultEdge?.rule).toBe(gateRules[1]);
    expect(findEdge(g, "gateNode", "seg2")?.rule).toBe(gateRules[2]);
  });

  it("leaves surplus gate slots ALWAYS when rules run out", () => {
    const gateRules = [have("only")];
    const g = interpretTemplate(t, { gateRules, progressionCount: 2 }, {}, new Rng("gb2").fork("i"));
    expect(findEdge(g, "seg", "gateNode")?.rule).toBe(gateRules[0]);
    // the vault entrance and the gateNode->seg2 edge had no rule to bind → stay open
    const vaultEdge = g.edges.find((e) => e.from === "seg" && e.to.startsWith("seg~vault"));
    expect(vaultEdge?.rule).toBe(ALWAYS);
    expect(findEdge(g, "gateNode", "seg2")?.rule).toBe(ALWAYS);
  });
});

describe("template interpretation — loops", () => {
  const base: ReachTemplate = {
    id: "loops",
    criticalPath: ["hub", "s1", "s2", "s3", "cap", "term"],
    nodes: {
      hub: { role: "hub", slots: { min: 6, max: 6 } },
      s1: { role: "segment", slots: { min: 1, max: 1 } },
      s2: { role: "segment", slots: { min: 1, max: 1 } },
      s3: { role: "segment", slots: { min: 1, max: 1 } },
      cap: { role: "capstone", slots: { min: 1, max: 1 } },
      term: { role: "terminal", slots: { min: 1, max: 1 } },
    },
    branches: [],
    gating: { lockFraction: 0, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
    loops: { guaranteeAtLeastOne: true, density: 0 },
  };
  const cp = base.criticalPath;
  const backEdges = (g: MissionGraph): number =>
    g.edges.filter((e) => {
      const fi = cp.indexOf(e.from);
      const ti = cp.indexOf(e.to);
      return fi >= 0 && ti >= 0 && ti < fi;
    }).length;

  it("guarantees at least one back-edge", () => {
    for (let seed = 0; seed < 20; seed++) {
      const g = interpretTemplate(base, { gateRules: [], progressionCount: 3 }, {}, new Rng(`l-${seed}`).fork("i"));
      expect(backEdges(g)).toBeGreaterThanOrEqual(1);
    }
  });

  it("density 1 closes more loops on average than density 0", () => {
    let d0 = 0;
    let d1 = 0;
    const N = 200;
    for (let seed = 0; seed < N; seed++) {
      const g0 = interpretTemplate({ ...base, loops: { guaranteeAtLeastOne: true, density: 0 } }, { gateRules: [], progressionCount: 3 }, {}, new Rng(`d0-${seed}`).fork("i"));
      const g1 = interpretTemplate({ ...base, loops: { guaranteeAtLeastOne: true, density: 1 } }, { gateRules: [], progressionCount: 3 }, {}, new Rng(`d1-${seed}`).fork("i"));
      d0 += backEdges(g0);
      d1 += backEdges(g1);
    }
    expect(d1 / N).toBeGreaterThan(d0 / N);
  });
});

describe("template interpretation — bootstrap invariant", () => {
  it("throws GenError when a template under-provisions always-reachable slots", () => {
    const t: ReachTemplate = {
      id: "starved",
      criticalPath: ["hub", "seg"],
      nodes: {
        hub: { role: "hub", slots: { min: 1, max: 1 } },
        seg: { role: "segment", slots: { min: 1, max: 1 } },
      },
      branches: [],
      gating: { lockFraction: 1, compoundChance: 0, keepEntryOpen: false, keepExitOpen: false },
      loops: { guaranteeAtLeastOne: false, density: 0 },
    };
    let err: GenError | undefined;
    try {
      interpretTemplate(t, { gateRules: [have("x")], progressionCount: 3 }, {}, new Rng("bs").fork("i"));
    } catch (e) {
      err = e as GenError;
    }
    expect(err).toBeInstanceOf(GenError);
    expect(err?.code).toBe("template.bootstrap");
    expect(err?.message).toContain("starved");
  });
});

describe("validateGraph — one-way stranding", () => {
  it("throws when a region is reachable only from an unreachable predecessor via a one-way", () => {
    const g: MissionGraph = {
      regions: [
        { id: "start", role: "hub" },
        { id: "ghost", role: "segment" },
        { id: "pocket", role: "segment" },
      ],
      // pocket is only entered by a one-way from ghost, and ghost has no inbound edge at all.
      edges: [{ from: "ghost", to: "pocket", rule: ALWAYS, oneWay: true }],
      flags: [],
      locations: [],
      start: "start",
    };
    expect(() => validateGraph(g, heldOf([]))).toThrow(GenError);
  });
});

describe("template pool", () => {
  const tA: ReachTemplate = {
    id: "A",
    criticalPath: ["hub", "term"],
    nodes: { hub: { role: "hub", slots: { min: 3, max: 3 } }, term: { role: "terminal", slots: { min: 1, max: 1 } } },
    branches: [],
    gating: { lockFraction: 0, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
    loops: { guaranteeAtLeastOne: false, density: 0 },
  };
  const tB: ReachTemplate = { ...tA, id: "B" };
  const pool: ReachTemplatePool = { poolAt: () => [{ template: tA, weight: 3 }, { template: tB, weight: 1 }] };

  it("draws deterministically per seed and respects weights", () => {
    expect(drawTemplate(pool, 0, new Rng("x").fork("t")).id).toBe(drawTemplate(pool, 0, new Rng("x").fork("t")).id);
    const counts = { A: 0, B: 0 };
    for (let seed = 0; seed < 4000; seed++) counts[drawTemplate(pool, 0, new Rng(`p-${seed}`).fork("t")).id as "A" | "B"]++;
    // ~3:1 favoring A
    expect(counts.A).toBeGreaterThan(counts.B * 2);
  });
});
