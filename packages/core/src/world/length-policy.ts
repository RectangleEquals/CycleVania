/**
 * How many Reaches a World has (`WorldLengthPolicy`, drawn once as `L`) and how
 * many Areas a Reach has (`AreaCountConfig`, drawn per Reach). Both are the same
 * ranged, weighted-draw pattern; uniform by default.
 */

import { Rng } from "../math/index.js";
import type { ReachModifierId } from "./modifiers.js";

export interface WorldLengthPolicy {
  min: number;
  max?: number; // omit for a genuinely open-ended World (no virtual schedule / final sweep)
  weights?: (n: number) => number;
}

export interface AreaCountConfig {
  min: number;
  max: number;
  weights?: (n: number, ctx: { reachIndex: number; chosenModifiers: ReachModifierId[]; finalCeiling: number }) => number;
}

export function drawRanged(rng: Rng, min: number, max: number, weights?: (n: number) => number): number {
  if (max <= min) return min;
  const entries: { item: number; weight: number }[] = [];
  for (let n = min; n <= max; n++) entries.push({ item: n, weight: weights ? weights(n) : 1 });
  return rng.weighted(entries);
}

/** Draw the World's total Reach count, once, from the seed. `undefined` = unbounded. */
export function drawWorldLength(seed: string, policy?: WorldLengthPolicy): number | undefined {
  if (!policy || policy.max === undefined) return undefined;
  return drawRanged(new Rng(seed).fork("world-length"), policy.min, policy.max, policy.weights);
}

export function drawAreaCount(
  cfg: AreaCountConfig,
  ctx: { reachIndex: number; chosenModifiers: ReachModifierId[]; finalCeiling: number },
  rng: Rng,
): number {
  return drawRanged(rng, cfg.min, cfg.max, cfg.weights ? (n) => cfg.weights?.(n, ctx) ?? 1 : undefined);
}
