/**
 * Exploration-reward placement weights. These bias WHERE a progression item lands
 * among the already-safe candidate Locations — they can never make an unsafe
 * choice (unsafe Locations are never in the candidate set), so they never threaten
 * solvability. Net effect: never in the entry Region, at most one per Region,
 * biased toward vaults / behind gates / deeper, spread across spheres.
 */

import type { LocationId, NodeRole, RegionId } from "../graph/index.js";

export interface PlacementWeightConfig {
  /** Multiplier for entry-Region Locations. Default 0 → progression never there. */
  entrySpaceWeight: number;
  /** Deeper Locations weighted up: factor = (1 + depthRank) ** depthExponent. */
  depthExponent: number;
  /** Multiplier for vault-role Regions. */
  vaultBonus: number;
  /** Multiplier when the Location sits past ≥ 1 gated edge. */
  behindGateBonus: number;
  /** Max progression items per Region (soft — relaxed only if candidates run out). */
  perRegionCap: number;
  /** Multiplier when the Location's depth differs from the last placement's. */
  sphereSpreadBonus: number;
}

export const DEFAULT_PLACEMENT_WEIGHTS: PlacementWeightConfig = {
  entrySpaceWeight: 0,
  depthExponent: 1.5,
  vaultBonus: 2.0,
  behindGateBonus: 1.5,
  perRegionCap: 1,
  sphereSpreadBonus: 1.5,
};

export interface CandidateLocation {
  locationId: LocationId;
  regionId: RegionId;
  regionRole: NodeRole;
  /** Structural BFS depth from start (start = 0). */
  depthRank: number;
  /** Minimum number of gated edges on a path from start. */
  behindGateCount: number;
  /** Is this the entry (start) Region? */
  isEntry: boolean;
}

export interface WeightContext {
  placedInRegion: ReadonlyMap<RegionId, number>;
  /** Depth of the last placement, for sphere-spreading. */
  lastDepthHint?: number;
}

/** Weight of a candidate Location; 0 removes it from the weighted draw. */
export function locationWeight(loc: CandidateLocation, ctx: WeightContext, cfg: PlacementWeightConfig): number {
  if ((ctx.placedInRegion.get(loc.regionId) ?? 0) >= cfg.perRegionCap) return 0;
  let w = Math.pow(1 + loc.depthRank, cfg.depthExponent);
  if (loc.regionRole === "vault") w *= cfg.vaultBonus;
  if (loc.behindGateCount >= 1) w *= cfg.behindGateBonus;
  if (loc.isEntry) w *= cfg.entrySpaceWeight;
  if (ctx.lastDepthHint !== undefined && loc.depthRank !== ctx.lastDepthHint) w *= cfg.sphereSpreadBonus;
  return w;
}
