/**
 * The complexity budget — a Reach's difficulty/size ceiling. Four terms:
 *   1. a tiered base curve (plateaus every TIER_SIZE Reaches + a gentle slope),
 *   2. seeded entropy jitter (triangular — central values likelier),
 *   3. a lookbehind pull toward the realized previous ceiling (no whiplash),
 *   4. player-chosen Reach-modifier deltas (additive + multiplicative), clamped by
 *      a second, higher safety ceiling independent of how many stack.
 *
 * Deltas, never overwrites. Modifier complexity dials are scalars applied to the
 * ceiling number (the redesign's worked table is illustrative, not a fixed
 * formula — the properties tested are tier plateaus, escalation, and the clamps).
 */

import { clamp, lerp, type Rng } from "../math/index.js";
import type { ReachModifierDef } from "./modifiers.js";

export interface ComplexityConfig {
  BaseCeiling: number;
  K_MUL: number;
  K_ADD: number;
  TIER_SIZE: number;
  JITTER_FRAC: number;
  LOOKBEHIND_PULL: number;
  MIN_CEILING: number;
  HARD_MAX: number;
  ABSOLUTE_HARD_MAX: number;
}

export const DEFAULT_COMPLEXITY: ComplexityConfig = {
  BaseCeiling: 100,
  K_MUL: 0.45,
  K_ADD: 5,
  TIER_SIZE: 3,
  JITTER_FRAC: 0.08,
  LOOKBEHIND_PULL: 0.35,
  MIN_CEILING: 60,
  HARD_MAX: 400,
  ABSOLUTE_HARD_MAX: 600,
};

export interface BaselineConfig {
  base: number;
  K_MUL: number;
  K_ADD: number;
}

export const DEFAULT_HAZARD_BASELINE: BaselineConfig = { base: 0.2, K_MUL: 0.25, K_ADD: 0.02 };
export const DEFAULT_REWARD_BASELINE: BaselineConfig = { base: 1, K_MUL: 0.2, K_ADD: 0.05 };

export function reachLevel(i: number, cfg: ComplexityConfig): number {
  return Math.floor(i / cfg.TIER_SIZE);
}

export function expectedCeiling(i: number, cfg: ComplexityConfig): number {
  return cfg.BaseCeiling * (1 + cfg.K_MUL * reachLevel(i, cfg)) + cfg.K_ADD * i;
}

/** The realized ceiling for Reach i, with lookbehind + seeded entropy jitter. */
export function actualCeiling(i: number, realizedPrev: number | undefined, worldRng: Rng, cfg: ComplexityConfig): number {
  const expected = expectedCeiling(i, cfg);
  const jitter = worldRng.fork(`reach-entropy:${i}`).triangular(-cfg.JITTER_FRAC, cfg.JITTER_FRAC) * expected;
  const anchored = lerp(expected, realizedPrev ?? expected, cfg.LOOKBEHIND_PULL);
  return clamp(anchored + jitter, cfg.MIN_CEILING, cfg.HARD_MAX);
}

/** Fold player-chosen modifier deltas on top, clamped by the absolute safety ceiling. */
export function finalCeiling(actual: number, mods: readonly ReachModifierDef[], cfg: ComplexityConfig): number {
  let additive = 0;
  let multiplier = 1;
  for (const m of mods) {
    additive += m.dials.complexity?.additive ?? 0;
    multiplier *= 1 + (m.dials.complexity?.multiplier ?? 0);
  }
  return clamp(actual * multiplier + additive, cfg.MIN_CEILING, cfg.ABSOLUTE_HARD_MAX);
}

/** An ambient, depth-driven baseline (hazard or reward), same tier-curve shape. */
export function baselineAt(i: number, cfg: BaselineConfig, tierSize: number): number {
  return cfg.base * (1 + cfg.K_MUL * Math.floor(i / tierSize)) + cfg.K_ADD * i;
}
