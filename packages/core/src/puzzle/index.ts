export type {
  PuzzleScope,
  PuzzleClass,
  EdgeSpec,
  ItemSpec,
  PuzzleOutcome,
  PuzzleDef,
  PuzzleInstance,
  PuzzleEconomyConfig,
} from "./puzzle-def.js";
export { DEFAULT_PUZZLE_ECONOMY } from "./puzzle-def.js";
export { SHIPPED_RECIPES, SHIPPED_RECIPES_BY_ID } from "./recipes.js";
export type { SpatialRecipeDef } from "./recipes.js";
export {
  capabilityLock,
  switchLock,
  panelArray,
  arenaLockdown,
  collectathon,
  bossGate,
  bonusReward,
} from "./lock-vocabulary.js";
export { instantiate, outcomeGrantsCapability, outcomeSetsFlag, outcomeOpensEdge } from "./outcomes.js";
export { puzzleEntry } from "./puzzle-scheduler.js";
