/**
 * The eligibility curve shared by the gadget/puzzle schedulers (M04) and the
 * virtual schedule (this milestone). A logistic in ReachLevel shifted right by
 * `powerWeight`: low-power ≈ eligible immediately, high-power ≈ needs several
 * ReachLevels. A pity window forces eligibility to 1 once its bound passes.
 *
 * M04 extends this module with `scheduleDraw`; the curve itself lives here so the
 * virtual schedule can use it without a forward dependency.
 */

import type { Rng } from "../math/index.js";
import type { SchedulableEntry } from "./virtual-schedule.js";

export const MAX_LEVEL_SHIFT = 6;
export const SOFTNESS = 1.25;
/** Weight multiplier applied to an entry planned for this exact slot. */
export const PLAN_BONUS = 8;

export function eligibility(
  reachLevel: number,
  powerWeight: number,
  levelsSinceEligible = 0,
  guarantee?: { withinReachLevels: number },
): number {
  if (guarantee && levelsSinceEligible >= guarantee.withinReachLevels) return 1;
  const midpoint = powerWeight * MAX_LEVEL_SHIFT;
  return 1 / (1 + Math.exp(-(reachLevel - midpoint) / SOFTNESS));
}

export interface ScheduleContext {
  reachLevel: number;
  reachIndex: number;
  isFinalReach: boolean;
  /** id → grants so far across prior Reaches (0 = not yet placed). */
  placedLevels: ReadonlyMap<string, number>;
  /** id → ReachLevel it first became eligible (default 0). */
  firstEligibleLevel: ReadonlyMap<string, number>;
  /** Strong bias for entries planned for this exact slot. */
  virtualPlan?: ReadonlyMap<string, number>;
}

export interface ScheduleDrawResult {
  chosen: string[];
  pityForced: string[];
  sweepForced: string[];
}

/**
 * Draw a per-Reach subset from an eligible pool. Normal entries are drawn by
 * weight (eligibility × plan bonus) without replacement, up to a seeded count in
 * [min, max]. Pity-forced entries (guarantee window elapsed) are placed on top,
 * exempt from `max`. On the final Reach, every remaining entry is sweep-forced.
 * The three result sets are disjoint.
 */
export function scheduleDraw(
  pool: readonly SchedulableEntry[],
  count: { min: number; max: number },
  ctx: ScheduleContext,
  rng: Rng,
): ScheduleDrawResult {
  const levelsSince = (e: SchedulableEntry): number => ctx.reachLevel - (ctx.firstEligibleLevel.get(e.id) ?? 0);
  const nextLevel = (e: SchedulableEntry): number => (ctx.placedLevels.get(e.id) ?? 0) + 1;
  const isPity = (e: SchedulableEntry): boolean =>
    e.guarantee !== undefined && levelsSince(e) >= e.guarantee.withinReachLevels;

  const pityForced = pool.filter(isPity).map((e) => e.id);
  const pitySet = new Set(pityForced);

  const weightOf = (e: SchedulableEntry): number => {
    const base = eligibility(ctx.reachLevel, e.powerWeight(nextLevel(e)), levelsSince(e), e.guarantee);
    const bonus = ctx.virtualPlan?.get(e.id) === ctx.reachIndex ? PLAN_BONUS : 1;
    return base * bonus;
  };

  const live = pool.filter((e) => !pitySet.has(e.id)).map((e) => ({ e, weight: weightOf(e) }));
  const n = rng.int(count.min, count.max);
  const chosen: string[] = [];
  for (let k = 0; k < n && live.length > 0; k++) {
    const picked = rng.weighted(live.map((x) => ({ item: x, weight: x.weight > 0 ? x.weight : 1e-6 })));
    chosen.push(picked.e.id);
    live.splice(live.indexOf(picked), 1);
  }

  const done = new Set([...chosen, ...pityForced]);
  const sweepForced = ctx.isFinalReach ? pool.filter((e) => !done.has(e.id)).map((e) => e.id) : [];
  return { chosen, pityForced, sweepForced };
}
