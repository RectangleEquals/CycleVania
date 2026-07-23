import { describe, it, expect } from "vitest";
import { Rng } from "../math/index.js";
import { eligibility, scheduleDraw, type ScheduleContext } from "./scheduling.js";
import type { SchedulableEntry } from "./virtual-schedule.js";

const ctxAt = (reachLevel: number, extra: Partial<ScheduleContext> = {}): ScheduleContext => ({
  reachLevel,
  reachIndex: reachLevel,
  isFinalReach: false,
  placedLevels: new Map(),
  firstEligibleLevel: new Map(),
  ...extra,
});

describe("eligibility", () => {
  it("rises with ReachLevel and is lower for higher power", () => {
    expect(eligibility(0, 0.2)).toBeGreaterThan(eligibility(0, 0.9));
    expect(eligibility(8, 0.9)).toBeGreaterThan(eligibility(0, 0.9));
  });
  it("is forced to 1 once a pity window elapses", () => {
    expect(eligibility(2, 0.9, 2, { withinReachLevels: 2 })).toBe(1);
    expect(eligibility(1, 0.9, 1, { withinReachLevels: 2 })).toBeLessThan(1);
  });
});

describe("scheduleDraw distribution", () => {
  const pool: SchedulableEntry[] = [
    { id: "low", powerWeight: () => 0.2 },
    { id: "high", powerWeight: () => 0.9 },
  ];
  const runCounts = (reachLevel: number): { low: number; high: number } => {
    const counts = { low: 0, high: 0 };
    for (let seed = 0; seed < 400; seed++) {
      const { chosen } = scheduleDraw(pool, { min: 1, max: 1 }, ctxAt(reachLevel), new Rng(`s-${reachLevel}-${seed}`));
      for (const id of chosen) counts[id as "low" | "high"]++;
    }
    return counts;
  };

  it("favors low-power early but never makes high-power impossible", () => {
    const early = runCounts(0);
    expect(early.low).toBeGreaterThan(early.high);
    expect(early.high).toBeGreaterThanOrEqual(1); // rare but possible
  });
  it("makes high-power more likely at deeper ReachLevels", () => {
    expect(runCounts(8).high).toBeGreaterThan(runCounts(0).high);
  });
});

describe("scheduleDraw pity + sweep", () => {
  it("pity-forces a guaranteed entry exempt from max", () => {
    const pool: SchedulableEntry[] = [{ id: "g", powerWeight: () => 0.9, guarantee: { withinReachLevels: 2 } }];
    const { chosen, pityForced } = scheduleDraw(pool, { min: 0, max: 0 }, ctxAt(2), new Rng("p"));
    expect(chosen).toEqual([]); // max 0
    expect(pityForced).toEqual(["g"]); // still forced
  });

  it("final-Reach sweep force-places everything remaining", () => {
    const pool: SchedulableEntry[] = [
      { id: "a", powerWeight: () => 0.5 },
      { id: "b", powerWeight: () => 0.5 },
      { id: "c", powerWeight: () => 0.5 },
    ];
    const { chosen, sweepForced } = scheduleDraw(pool, { min: 1, max: 1 }, ctxAt(0, { isFinalReach: true }), new Rng("f"));
    expect(chosen.length).toBe(1);
    expect(new Set([...chosen, ...sweepForced])).toEqual(new Set(["a", "b", "c"])); // all placed
  });

  it("gives a large bonus to an entry planned for this exact slot", () => {
    const pool: SchedulableEntry[] = [
      { id: "planned", powerWeight: () => 0.9 },
      { id: "other", powerWeight: () => 0.9 },
    ];
    let planned = 0;
    for (let seed = 0; seed < 300; seed++) {
      const { chosen } = scheduleDraw(
        pool,
        { min: 1, max: 1 },
        ctxAt(0, { virtualPlan: new Map([["planned", 0]]) }),
        new Rng(`pl-${seed}`),
      );
      if (chosen[0] === "planned") planned++;
    }
    expect(planned).toBeGreaterThan(200); // plan bias dominates the tie
  });
});
