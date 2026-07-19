/**
 * @cyclevania/core — deterministic, renderer-free, data-driven procgen for
 * co-op metroidvanias. Public surface. See ./Docs for the full guide.
 */

// --- determinism math ---
export { Rng, fnv1a, shuffle, dsin, dcos, datan, datan2, reduce, yawFromDirection, yawBasis } from "./math/index.js";
export {
  add,
  sub,
  scale,
  mul,
  dot,
  cross,
  length,
  normalize,
  lerp3,
  vecEq,
  ZERO3,
  clamp,
  clamp01,
  lerp,
  invLerp,
  smoothstep,
  boxFromCenterHalf,
  boxCenter,
  boxSize,
  boxOverlap,
  boxContainsPoint,
  boxUnion,
  boxUnionAll,
} from "./math/index.js";
export type { Vec3, WorldBox } from "./math/index.js";

// --- access logic ---
export { ALWAYS, have, count, flag, not, and, or, evalRule, ruleCaps, ruleFlags, missingCaps, isOpen, CapSet, heldOf } from "./logic/index.js";
export type { Rule, Capability, Held } from "./logic/index.js";

// --- region graph + solver ---
export { reachableRegions, reachableLocations, computeSpheres, isSolvable, hasCycle, validateGraph } from "./graph/index.js";
export type { RegionGraph, RegionEdge, RegionId, LocationId, ProgressionItem, Placement, SphereResult, GraphValidation } from "./graph/index.js";

// --- fill ---
export { assumedFill, fillRemaining } from "./fill/index.js";
export type { AssumedFillOptions, ItemClass } from "./fill/index.js";

// --- template DSL ---
export { generateReach } from "./template/index.js";
export type { ReachTemplate, TemplateNode, BranchSpec, BranchEntrance, RegionRole, GenerateReachParams, GeneratedReach, ReachMeta } from "./template/index.js";

// --- registries (data injection) ---
export { defineRegistry, RULE_DSL, complexityFor, DEFAULT_COMPLEXITY, piecesForRole, kitRoles } from "./registries/index.js";
export type {
  Registry,
  RegistryInput,
  GridConfig,
  ComplexityConfig,
  ComplexityBudget,
  ComplexityMods,
  ItemDef,
  ItemCatalogInput,
  CapabilityProfile,
  CapabilityProfileGrants,
  CapabilityProfileBias,
  UseEffect,
  RuleDsl,
  RuleBuilder,
  ChallengeTemplate,
  LockDef,
  LockVocabularyInput,
  ResolvedLock,
  GeometryKit,
  KitPiece,
  AdjacencyRule,
  RoomArchetype,
  ConnectorArchetype,
  StyleDef,
} from "./registries/index.js";

// --- spatial primitives ---
export { cellCenter, cellMin, coordKey, classifyCell, boundaryFaces, faceNormal, oppositeFace, snapAngle, anglePalette, Occupancy } from "./spatial/index.js";
export type { Coord, CellRole, CellClass, Socket } from "./spatial/index.js";

// --- abstract descriptors (renderer-free output) ---
export { assembleWorld } from "./descriptors/index.js";
export type { ContentAnchor, GadgetPlacement, CellDescriptor, RoomDescriptor, ConnectorPlan, PortalSpec, AreaDescriptor, AreaLink, ReachDescriptor, WorldDescriptor } from "./descriptors/index.js";

// --- composers ---
export { composeRoom, composeArena, composeArea, composeReach, composeWorld, corridorGeometry } from "./composers/index.js";
export type { ComposeContext, RoomComposeParams, RoomShape, SocketRequest, ContentRequest, AreaComposeParams, PortalRequest, ComposeReachOptions, ReachResult, ComposeWorldOptions, WorldResult, ConnectorGeom } from "./composers/index.js";

// --- async orchestration ---
export { composeWorldAsync, composeReachAsync, CancellationToken, GenCancelled, GenerationHorizon } from "./orchestration/index.js";
export type { OrchestrationHooks, JobProgress } from "./orchestration/index.js";

// --- playtest simulator ---
export { buildSimWorld, neighbors, reachableAreaIds, initSim, cloneState, parseCommand, step, autosolve } from "./sim/index.js";
export type { SimWorld, SimArea, SimLink, SimGadget, SimItemInfo, SimState, Command, SimResult } from "./sim/index.js";

// --- shared type aliases ---
export type { Traversal, SnapPolicy, ConnectorKind, CellFace, SocketKind, RoomKind } from "./types.js";
