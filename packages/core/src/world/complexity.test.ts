import { describe, it, expect } from "vitest";
import { Rng } from "../math/index.js";
import {
  DEFAULT_COMPLEXITY,
  reachLevel,
  expectedCeiling,
  actualCeiling,
  finalCeiling,
  baselineAt,
  DEFAULT_HAZARD_BASELINE,
} from "./complexity.js";
import type { ReachModifierDef } from "./modifiers.js";

const cfg = DEFAULT_COMPLEXITY;

describe("complexity formula", () => {
  it("reachLevel tiers by TIER_SIZE", () => {
    expect(reachLevel(0, cfg)).toBe(0);
    expect(reachLevel(2, cfg)).toBe(0);
    expect(reachLevel(3, cfg)).toBe(1);
    expect(reachLevel(8, cfg)).toBe(2);
    expect(reachLevel(15, cfg)).toBe(5);
  });

  it("expectedCeiling is non-decreasing and escalates across tiers", () => {
    for (let i = 1; i < 30; i++) expect(expectedCeiling(i, cfg)).toBeGreaterThanOrEqual(expectedCeiling(i - 1, cfg));
    expect(expectedCeiling(9, cfg)).toBeGreaterThan(expectedCeiling(0, cfg));
  });

  it("actualCeiling is deterministic and stays within [MIN, HARD_MAX]", () => {
    const rng = () => new Rng("world-seed");
    expect(actualCeiling(4, 150, rng(), cfg)).toBe(actualCeiling(4, 150, rng(), cfg));
    for (let i = 0; i < 30; i++) {
      const v = actualCeiling(i, i > 0 ? 150 : undefined, new Rng(`s${i}`), cfg);
      expect(v).toBeGreaterThanOrEqual(cfg.MIN_CEILING);
      expect(v).toBeLessThanOrEqual(cfg.HARD_MAX);
    }
  });

  it("finalCeiling never exceeds ABSOLUTE_HARD_MAX no matter how many modifiers stack", () => {
    const big: ReachModifierDef[] = Array.from({ length: 6 }, (_, k) => ({
      id: `big${k}`,
      riskWeight: 1,
      rewardWeight: 1,
      minDepth: 0,
      dials: { complexity: { multiplier: 1, additive: 100 } },
    }));
    expect(finalCeiling(cfg.HARD_MAX, big, cfg)).toBeLessThanOrEqual(cfg.ABSOLUTE_HARD_MAX);
    expect(finalCeiling(200, [], cfg)).toBe(200);
  });

  it("rolling-average ceiling trends up over 60 Reaches (never asserted on adjacent pairs)", () => {
    const worldRng = new Rng("trend");
    const seq: number[] = [];
    let prev: number | undefined;
    for (let i = 0; i < 60; i++) {
      const v = actualCeiling(i, prev, worldRng, cfg);
      seq.push(v);
      prev = v;
    }
    const avg = (a: number[]): number => a.reduce((s, x) => s + x, 0) / a.length;
    expect(avg(seq.slice(30, 60))).toBeGreaterThan(avg(seq.slice(0, 30)));
  });

  it("baselineAt escalates with depth", () => {
    expect(baselineAt(10, DEFAULT_HAZARD_BASELINE, cfg.TIER_SIZE)).toBeGreaterThan(baselineAt(0, DEFAULT_HAZARD_BASELINE, cfg.TIER_SIZE));
  });
});
