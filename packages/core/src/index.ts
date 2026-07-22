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
export { defineRegistry, RULE_DSL, complexityFor, DEFAULT_COMPLEXITY, piecesForRole, kitRoles, DEFAULT_HULL_ARCHETYPES, DEFAULT_BIOME, DEFAULT_FIDELITY } from "./registries/index.js";
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
  HullArchetypeDef,
  HullArchetypeRegistry,
  BiomePack,
  BiomeRegistry,
  FidelityConfig,
} from "./registries/index.js";

// --- spatial primitives ---
export { cellCenter, cellMin, coordKey, classifyCell, boundaryFaces, faceNormal, oppositeFace, socketBasis, snapAngle, anglePalette, Occupancy } from "./spatial/index.js";
export type { Coord, CellRole, CellClass, Socket, SocketBasis, SocketMeta } from "./spatial/index.js";

// --- volume (SDF) + geometry (dual contouring, kit, collision) + layout ---
export { sphere, ellipsoid, box, roundBox, capsule, plane, union, subtract, intersect, smoothUnion, displace, sdfNormal, hull, HULL_ARCHETYPES, catmullRom, splineTube, connectorTube, composeAreaField } from "./volume/index.js";
export type { Sdf, HullParams, HullArchetype, AreaField } from "./volume/index.js";
export { quantizeNormal, isQuantized, dualContour, meshToKit, occupancyGrid, cellOf, isSolidAt, collideSphere, dressArea } from "./geometry/index.js";
export type { Mesh, GeneratedKit, GeneratedPiece, PieceInstance, PieceMeta, SurfaceKind, KitOptions, OccupancyGrid, DressingAnchor } from "./geometry/index.js";
export { forceLayout } from "./layout/index.js";
export type { LayoutNode, LayoutEdge, LayoutOptions, LayoutResult } from "./layout/index.js";

// --- abstract descriptors (renderer-free output) ---
export { assembleWorld } from "./descriptors/index.js";
export type { ContentAnchor, GadgetPlacement, CellDescriptor, RoomDescriptor, ConnectorPlan, PortalSpec, AreaDescriptor, AreaLink, ReachDescriptor, WorldDescriptor, OccupancyData } from "./descriptors/index.js";

// --- composers ---
export { composeRoom, composeArena, composeArea, composeReach, composeWorld, corridorGeometry, reachGadgets, selectReachGadgets, itemPower, buildAreaGeometry } from "./composers/index.js";
export type { ComposeContext, RoomComposeParams, RoomShape, SocketRequest, ContentRequest, AreaComposeParams, PortalRequest, ComposeReachOptions, ReachResult, ComposeWorldOptions, WorldResult, ConnectorGeom } from "./composers/index.js";

// --- async orchestration ---
export { composeWorldAsync, composeReachAsync, CancellationToken, GenCancelled, GenerationHorizon } from "./orchestration/index.js";
export type { OrchestrationHooks, JobProgress } from "./orchestration/index.js";

// --- playtest simulator ---
export { buildSimWorld, neighbors, reachableAreaIds, initSim, cloneState, parseCommand, step, autosolve } from "./sim/index.js";
export type { SimWorld, SimArea, SimLink, SimGadget, SimItemInfo, SimState, Command, SimResult } from "./sim/index.js";

// --- shared type aliases ---
export type { Traversal, SnapPolicy, ConnectorKind, CellFace, SocketKind, RoomKind } from "./types.js";
