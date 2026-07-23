/**
 * Budget → dials. An Area's complexity slice + the aggregated capability buckets
 * become concrete L2 dials (space count, verticality, outdoor chance, …). The
 * world-shaping loop lands here: `zSpread` grows sub-linearly with the z buckets
 * (`base + K * sqrt(zUp)`) so worlds stay lateral-dominant unless retuned.
 */

import { Rng } from "../math/index.js";
import type { NodeRole } from "../graph/index.js";

export interface SpaceBudget {
  volumeCells: number;
  polyAllowance: number;
  degree: { min: number; max: number };
}

export interface AreaDials {
  spaceCount: number;
  zSpread: number;
  loopDensity: number;
  outdoorChance: number;
  largeSpaceChance: number;
  secretFraction: number;
  hazardDensity: number;
}

export interface AreaDialConfig {
  spaceCost: number; // budget units per Space
  minSpaces: number;
  maxSpaces: number;
  zBase: number;
  K_Z: number; // sqrt-shaped verticality response
  baseOutdoorChance: number;
  baseLargeChance: number;
  baseSecretFraction: number;
}

export const DEFAULT_AREA_DIALS: AreaDialConfig = {
  spaceCost: 22,
  minSpaces: 1,
  maxSpaces: 12,
  zBase: 1,
  K_Z: 1.4,
  baseOutdoorChance: 0.18,
  baseLargeChance: 0.15,
  baseSecretFraction: 0.15,
};

/** Split a Reach's ceiling across N Areas with uneven, role-weighted shares. */
export function splitReachBudget(ceiling: number, roles: readonly NodeRole[], rng: Rng): number[] {
  const weightOf = (r: NodeRole): number => (r === "capstone" ? 1.8 : r === "hub" ? 1.3 : r === "vault" ? 0.7 : 1);
  const weights = roles.map(weightOf).map((w) => w * (0.75 + rng.next() * 0.5));
  const total = weights.reduce((s, w) => s + w, 0) || 1;
  return weights.map((w) => (w / total) * ceiling);
}

export function deriveAreaDials(
  budgetSlice: number,
  buckets: Readonly<Record<string, number>>,
  role: NodeRole,
  cfg: AreaDialConfig,
  rng: Rng,
): AreaDials {
  const zUp = Math.max(0, buckets["traversal.zUp"] ?? 0);
  const zDown = Math.max(0, buckets["traversal.zDown"] ?? 0);
  const zSpread = cfg.zBase + cfg.K_Z * Math.sqrt(zUp + zDown);

  const roleScale = role === "capstone" ? 0.5 : role === "vault" ? 0.6 : role === "hub" ? 1.2 : 1;
  const raw = Math.round((budgetSlice / cfg.spaceCost) * roleScale);
  const spaceCount = Math.max(cfg.minSpaces, Math.min(cfg.maxSpaces, raw + (rng.chance(0.4) ? 1 : 0)));

  const budgetT = Math.min(1, budgetSlice / 400);
  return {
    spaceCount,
    zSpread,
    loopDensity: Math.min(1, 0.1 + 0.3 * budgetT),
    outdoorChance: cfg.baseOutdoorChance,
    largeSpaceChance: cfg.baseLargeChance + 0.15 * budgetT,
    secretFraction: cfg.baseSecretFraction,
    hazardDensity: 0.2 + 0.3 * budgetT,
  };
}
