export {
  DEFAULT_COMPLEXITY,
  DEFAULT_HAZARD_BASELINE,
  DEFAULT_REWARD_BASELINE,
  reachLevel,
  expectedCeiling,
  actualCeiling,
  finalCeiling,
  baselineAt,
} from "./complexity.js";
export type { ComplexityConfig, BaselineConfig } from "./complexity.js";
export { validateModifierChoice, resolveEconomy, structureNudges } from "./modifiers.js";
export type { ReachModifierId, ReachModifierDef, ReachModifierPolicy, DialPatch } from "./modifiers.js";
export { requestIdentity } from "./reach-request.js";
export type { ReachRequest, ReachRequestRecord } from "./reach-request.js";
export type { ReachPortal } from "./portals.js";
export { eligibility, scheduleDraw, MAX_LEVEL_SHIFT, SOFTNESS, PLAN_BONUS } from "./scheduling.js";
export type { ScheduleContext, ScheduleDrawResult } from "./scheduling.js";
export { drawWorldLength, drawAreaCount, drawRanged } from "./length-policy.js";
export type { WorldLengthPolicy, AreaCountConfig } from "./length-policy.js";
export { computeVirtualSchedule } from "./virtual-schedule.js";
export type { SchedulableEntry } from "./virtual-schedule.js";
export { createWorld, WorldComposer, verbatimSelector } from "./world-composer.js";
export { createRegistrySelector, worldFromRegistry } from "./content-selector.js";
export type { WorldFromRegistryOptions } from "./content-selector.js";
export type {
  WorldConfig,
  ContentSelector,
  SelectionContext,
  SelectionResult,
  ReachResult,
  ReachMeta,
  ReachEnvelopePreview,
  GenPhase,
} from "./world-composer.js";
