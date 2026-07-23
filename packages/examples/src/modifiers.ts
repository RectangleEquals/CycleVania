/**
 * Example Reach modifiers — player-chosen risk/reward dials, with a depth-driven
 * optional→mandatory ramp.
 */

import type { ReachModifierDef, ReachModifierPolicy } from "@cyclevania/core";

export const MODIFIERS: ReachModifierDef[] = [
  { id: "cramped", riskWeight: 0.3, rewardWeight: 0.3, minDepth: 0, dials: { complexity: { multiplier: 0.15 } }, tags: ["size"] },
  { id: "sprawling", riskWeight: 0.4, rewardWeight: 0.4, minDepth: 0, dials: { complexity: { additive: 30 } }, tags: ["size"], excludesTags: ["size"] },
  { id: "loot-rich", riskWeight: 0.2, rewardWeight: 0.8, minDepth: 0, dials: { reward: { lootTierBonus: 1, bonusLocations: 2 } } },
  { id: "hazardous", riskWeight: 0.6, rewardWeight: 0.5, minDepth: 3, dials: { hazard: { densityMul: 1.6 } } },
  { id: "labyrinthine", riskWeight: 0.5, rewardWeight: 0.4, minDepth: 3, dials: { structure: { extraLoopChance: 0.4, extraBranchChance: 0.3 } } },
  { id: "vertiginous", riskWeight: 0.6, rewardWeight: 0.5, minDepth: 5, dials: { complexity: { multiplier: 0.2 } }, tags: ["extreme"] },
  { id: "grueling", riskWeight: 0.9, rewardWeight: 0.9, minDepth: 8, dials: { complexity: { multiplier: 0.4, additive: 40 }, hazard: { densityMul: 2 } }, tags: ["extreme"] },
  { id: "cataclysmic", riskWeight: 1, rewardWeight: 1, minDepth: 12, dials: { complexity: { multiplier: 0.6 }, gadgetEconomy: { max: 1 } }, tags: ["extreme"], excludesTags: ["extreme"] },
];

export const MODIFIER_POLICY: ReachModifierPolicy = {
  poolAt: (depth) => MODIFIERS.filter((m) => m.minDepth <= depth),
  requiredRange: (depth) => (depth >= 15 ? { min: 2, max: 3 } : depth >= 8 ? { min: 1, max: 3 } : { min: 0, max: 2 }),
};
