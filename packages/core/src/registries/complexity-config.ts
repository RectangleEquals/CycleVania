/**
 * Depth-driven complexity budget. A smoothstep S-curve maps a normalized depth to
 * a ceiling, then plateaus — the spatial analog of a difficulty pacing curve.
 * Linearity gets rarer with depth but never vanishes (layout samples AROUND these
 * means with seeded variance). The curve knobs are DATA so games can retune.
 */

import { clamp01, lerp, smoothstep } from "../math/curve.js";

export interface ComplexityConfig {
  /** Depth at which the curve reaches its ceiling. */
  depthCeiling: number;
  footprint: [number, number];
  roomCount: [number, number];
  loopChance: [number, number];
  extraCycles: [number, number];
  roomSizeMax: [number, number];
  mazeFactor: [number, number];
  /** Vertical spread (0 = flat/lateral). Diminishing returns keep it modest. */
  zSpread: [number, number];
}

export interface ComplexityBudget {
  c: number; // 0..1 curve scalar
  depth: number;
  footprint: number;
  roomCount: number;
  loopChance: number;
  extraCycles: number;
  roomSizeMax: number;
  mazeFactor: number;
  zSpread: number;
}

export interface ComplexityMods {
  /** Shift the normalized depth (e.g. an Omen that biases openness). */
  depthBias?: number;
}

export const DEFAULT_COMPLEXITY: ComplexityConfig = {
  depthCeiling: 40,
  footprint: [28, 92],
  roomCount: [3, 9],
  loopChance: [0.1, 0.9],
  extraCycles: [0, 3],
  roomSizeMax: [14, 34],
  mazeFactor: [0.15, 0.9],
  zSpread: [0, 0.6],
};

export function complexityFor(
  depth: number,
  cfg: ComplexityConfig = DEFAULT_COMPLEXITY,
  mods: ComplexityMods = {},
): ComplexityBudget {
  const t = clamp01(depth / cfg.depthCeiling + (mods.depthBias ?? 0));
  const c = smoothstep(t);
  const L = (r: [number, number]): number => lerp(r[0], r[1], c);
  return {
    c,
    depth,
    footprint: L(cfg.footprint),
    roomCount: Math.round(L(cfg.roomCount)),
    loopChance: L(cfg.loopChance),
    extraCycles: L(cfg.extraCycles),
    roomSizeMax: L(cfg.roomSizeMax),
    mazeFactor: L(cfg.mazeFactor),
    zSpread: L(cfg.zSpread),
  };
}
