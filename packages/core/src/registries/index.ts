export type { GridConfig } from "./grid-config.js";
export {
  DEFAULT_COMPLEXITY,
  complexityFor,
} from "./complexity-config.js";
export type { ComplexityConfig, ComplexityBudget, ComplexityMods } from "./complexity-config.js";
export type {
  ItemDef,
  ItemClass,
  ItemCatalogInput,
  CapabilityProfile,
  CapabilityProfileGrants,
  CapabilityProfileBias,
  UseEffect,
} from "./item-catalog.js";
export { RULE_DSL } from "./lock-vocabulary.js";
export type {
  RuleDsl,
  RuleBuilder,
  ChallengeTemplate,
  LockDef,
  LockVocabularyInput,
  ResolvedLock,
} from "./lock-vocabulary.js";
export { piecesForRole, kitRoles } from "./geometry-kit.js";
export type { GeometryKit, KitPiece, AdjacencyRule } from "./geometry-kit.js";
export type { RoomArchetype } from "./room-archetypes.js";
export type { ConnectorArchetype } from "./connector-archetypes.js";
export type { StyleDef } from "./style-registry.js";
export { defineRegistry } from "./registry.js";
export type { Registry, RegistryInput } from "./registry.js";
