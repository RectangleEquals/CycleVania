/**
 * Reach modifiers — player-chosen risk/reward dial patches applied to ONE Reach's
 * generation. A modifier is data-driven and opaque; CycleVania only reads its
 * dials. `DialPatch` deltas never overwrite. The optional→mandatory ramp lives in
 * `ReachModifierPolicy.requiredRange`.
 */

import { GenError } from "../errors.js";

export type ReachModifierId = string;

export interface DialPatch {
  /** Scalar deltas applied to the ceiling number. */
  complexity?: { additive?: number; multiplier?: number };
  gadgetEconomy?: Partial<{ min: number; max: number }>;
  puzzleEconomy?: Partial<{ min: number; max: number }>;
  reward?: Partial<{ lootTierBonus: number; bonusLocations: number }>;
  hazard?: Partial<{ densityMul: number }>;
  structure?: Partial<{ extraBranchChance: number; extraLoopChance: number }>;
  custom?: Record<string, number>;
}

export interface ReachModifierDef {
  id: ReachModifierId;
  riskWeight: number; // 0..1
  rewardWeight: number; // 0..1
  minDepth: number;
  dials: DialPatch;
  excludesTags?: string[];
  tags?: string[];
}

export interface ReachModifierPolicy {
  poolAt(depth: number): ReachModifierDef[];
  requiredRange(depth: number): { min: number; max: number };
}

/**
 * Validate a player's chosen modifier set at a depth: honors the required count
 * range, membership in the depth's pool, per-modifier minDepth, and tag exclusion.
 * Throws a typed `GenError` naming the offending id/field.
 */
export function validateModifierChoice(
  policy: ReachModifierPolicy,
  depth: number,
  chosen: readonly ReachModifierId[],
  catalog: ReadonlyMap<ReachModifierId, ReachModifierDef>,
): ReachModifierDef[] {
  const range = policy.requiredRange(depth);
  if (chosen.length < range.min || chosen.length > range.max) {
    throw new GenError(
      "modifier.count",
      `depth ${depth} requires ${range.min}..${range.max} modifiers, got ${chosen.length}`,
      { depth, range, got: chosen.length },
    );
  }
  const poolIds = new Set(policy.poolAt(depth).map((m) => m.id));
  const defs: ReachModifierDef[] = [];
  for (const id of chosen) {
    const def = catalog.get(id);
    if (!def) throw new GenError("modifier.unknown", `unknown modifier "${id}"`, { id });
    if (def.minDepth > depth) throw new GenError("modifier.depth", `modifier "${id}" not available before depth ${def.minDepth}`, { id, minDepth: def.minDepth, depth });
    if (!poolIds.has(id)) throw new GenError("modifier.not-in-pool", `modifier "${id}" is not in the pool at depth ${depth}`, { id, depth });
    defs.push(def);
  }
  for (const a of defs) {
    for (const b of defs) {
      if (a === b) continue;
      if (a.excludesTags && b.tags && a.excludesTags.some((t) => b.tags?.includes(t))) {
        throw new GenError("modifier.excluded", `modifier "${a.id}" excludes "${b.id}"`, { a: a.id, b: b.id });
      }
    }
  }
  return defs;
}

/** Resolve an economy {min,max} after modifier deltas then a request overwrite. */
export function resolveEconomy(
  base: { min: number; max: number },
  mods: readonly ReachModifierDef[],
  dial: "gadgetEconomy" | "puzzleEconomy",
  override?: Partial<{ min: number; max: number }>,
): { min: number; max: number } {
  let min = base.min;
  let max = base.max;
  for (const m of mods) {
    const d = m.dials[dial];
    if (d?.min !== undefined) min += d.min;
    if (d?.max !== undefined) max += d.max;
  }
  if (override?.min !== undefined) min = override.min;
  if (override?.max !== undefined) max = override.max;
  min = Math.max(1, Math.round(min));
  max = Math.max(min, Math.round(max));
  return { min, max };
}

/** Combined structural nudges from all chosen modifiers. */
export function structureNudges(mods: readonly ReachModifierDef[]): { extraBranchChance?: number; extraLoopChance?: number } {
  let branch = 0;
  let loop = 0;
  for (const m of mods) {
    branch += m.dials.structure?.extraBranchChance ?? 0;
    loop += m.dials.structure?.extraLoopChance ?? 0;
  }
  const out: { extraBranchChance?: number; extraLoopChance?: number } = {};
  if (branch > 0) out.extraBranchChance = branch;
  if (loop > 0) out.extraLoopChance = loop;
  return out;
}
