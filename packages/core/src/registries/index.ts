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
export { DEFAULT_HULL_ARCHETYPES } from "./hull-archetypes.js";
export type { HullArchetypeDef, HullArchetypeRegistry } from "./hull-archetypes.js";
export { DEFAULT_BIOME } from "./biome-pack.js";
export type { BiomePack, BiomeRegistry } from "./biome-pack.js";
export { DEFAULT_FIDELITY } from "./fidelity-config.js";
export type { FidelityConfig } from "./fidelity-config.js";
export type { RoomArchetype } from "./room-archetypes.js";
export type { ConnectorArchetype } from "./connector-archetypes.js";
export type { StyleDef } from "./style-registry.js";
export { defineRegistry } from "./registry.js";
export type { Registry, RegistryInput } from "./registry.js";
