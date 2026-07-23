/**
 * @cyclevania/core — deterministic, engine-agnostic, data-driven procgen for
 * co-op 3D metroidvanias. This is the single public API surface; it grows one
 * milestone at a time. See CycleVania/Docs/final for the design + build plan.
 */

// --- deterministic math foundation (M00) ---
export * from "./math/index.js";

// --- diagnostics channel (M00) ---
export { SILENT_SINK, MemorySink, Diag, DEFAULT_DIAGNOSTICS } from "./diagnostics.js";
export type { DiagLevel, DiagEvent, DiagnosticsSink, DiagnosticsConfig } from "./diagnostics.js";

// --- errors (M01 + M10 subclasses) ---
export { GenError, RegistryError, TemplateError, RequestError, PlacementError, BudgetError } from "./errors.js";

// --- orchestration (M10) ---
export { CancellationToken, GenCancelled, requestReachAsync, ProgressTracker, PHASE_WEIGHTS, GenerationHorizon, inlineWorker } from "./orchestration/index.js";
export type { OrchestrationHooks, GenProgress, HorizonPolicy, WorkerLike } from "./orchestration/index.js";

// --- access logic (M01) ---
export {
  ALWAYS,
  have,
  count,
  flag,
  not,
  and,
  or,
  evalRule,
  ruleCaps,
  ruleFlags,
  missingCaps,
  usesVolatileFlag,
  isOpen,
  CapSet,
  heldOf,
  heldFromData,
} from "./logic/index.js";
export type { Rule, CapabilityId, Held, HeldData } from "./logic/index.js";

// --- mission graph + solver (M01) ---
export {
  locationRegion,
  reachableRegions,
  reachableLocations,
  computeSpheres,
  isSolvable,
  hasCycle,
  validateGraph,
} from "./graph/index.js";
export type {
  RegionId,
  LocationId,
  NodeRole,
  ItemClass,
  Region,
  Edge,
  FlagDef,
  LocationDef,
  MissionGraph,
  Item,
  Placement,
  SphereResult,
} from "./graph/index.js";

// --- assumed fill (M02) ---
export { assumedFill, fillRemaining, DEFAULT_PLACEMENT_WEIGHTS, locationWeight } from "./fill/index.js";
export type { FillResult, PlacementWeightConfig, CandidateLocation, WeightContext } from "./fill/index.js";

// --- reach templates (M02) ---
export { interpretTemplate, drawTemplate } from "./template/index.js";
export type {
  ReachTemplate,
  TemplateNode,
  BranchSpec,
  ReachTemplatePool,
  SelectedContent,
  StructureNudges,
} from "./template/index.js";

// --- world layer: requests, modifiers, complexity, previews, schedules (M03) ---
export {
  createWorld,
  WorldComposer,
  verbatimSelector,
  DEFAULT_COMPLEXITY,
  DEFAULT_HAZARD_BASELINE,
  DEFAULT_REWARD_BASELINE,
  reachLevel,
  expectedCeiling,
  actualCeiling,
  finalCeiling,
  baselineAt,
  validateModifierChoice,
  resolveEconomy,
  structureNudges,
  requestIdentity,
  eligibility,
  drawWorldLength,
  drawAreaCount,
  drawRanged,
  computeVirtualSchedule,
  scheduleDraw,
} from "./world/index.js";
export type { ScheduleContext, ScheduleDrawResult } from "./world/index.js";

// --- capabilities & Facets (M04) ---
export { BUILTIN_BUCKETS, buildHeld, aggregateBuckets, activeTags, DEFAULT_GADGET_ECONOMY, gadgetEntry } from "./capability/index.js";
export type {
  BuiltinBucket,
  Bucket,
  FacetContext,
  MagnitudeFacet,
  TagFacet,
  ResourceFacet,
  Facet,
  CapabilityDef,
  GadgetDef,
  GadgetEconomyConfig,
} from "./capability/index.js";

// --- puzzles & locks (M04) ---
export {
  DEFAULT_PUZZLE_ECONOMY,
  SHIPPED_RECIPES,
  SHIPPED_RECIPES_BY_ID,
  capabilityLock,
  switchLock,
  panelArray,
  arenaLockdown,
  collectathon,
  bossGate,
  bonusReward,
  instantiate,
  outcomeGrantsCapability,
  outcomeSetsFlag,
  outcomeOpensEdge,
  puzzleEntry,
} from "./puzzle/index.js";
export type {
  PuzzleScope,
  PuzzleClass,
  EdgeSpec,
  ItemSpec,
  PuzzleOutcome,
  PuzzleDef,
  PuzzleInstance,
  PuzzleEconomyConfig,
  SpatialRecipeDef,
} from "./puzzle/index.js";

// --- registries (M04) ---
export { defineRegistry, DEFAULT_LOCK_PACING } from "./registries/index.js";
export type { RegistryInput, Registry, LockPacingConfig } from "./registries/index.js";

// --- registry-driven world (M04) ---
export { createRegistrySelector, worldFromRegistry } from "./world/index.js";
export type { WorldFromRegistryOptions } from "./world/index.js";

// --- simulation & autosolve (M09) ---
export { buildSimWorld, neighbors, reachableNodes, initSim, cloneState, parseCommand, step, autosolve } from "./sim/index.js";
export type { SimWorld, SimNode, SimLink, SimItemInfo, SimLocation, SimState, Command, SimResult, AutosolveResult } from "./sim/index.js";

// --- output descriptors (M08) + diagram/bundle (M13) ---
export {
  GENERATION_VERSION,
  assembleReach,
  assembleWorld,
  stableStringify,
  toTypedKit,
  missionGraphMermaid,
  reachMissionDiagram,
  ruleSummary,
  BUNDLE_KIND,
  makeBundle,
  checkReproduction,
  sniffPayload,
} from "./descriptors/index.js";
export type {
  Vec3Data,
  WorldBoxData,
  SocketData,
  ConnectorDescriptor,
  SpaceDescriptor,
  AreaDescriptor,
  MissionGraphData,
  PlacementEntry,
  ReachMetaData,
  ReachDescriptor,
  WorldMetaData,
  WorldDescriptor,
  TypedKit,
  TypedPiece,
  DiagramOptions,
  ReproductionBundle,
  BundleRegistryRef,
  MakeBundleArgs,
  ReproCheck,
  PayloadKind,
} from "./descriptors/index.js";

// --- finish pass L4 (M07) ---
export {
  DEFAULT_FIDELITY,
  snapNormal,
  dualContour,
  dualContourSteps,
  meshToKit,
  occupancyGrid,
  cellOf,
  isSolidAt,
  isWalkable,
  collideSphere,
  toOccupancyData,
  dressArea,
  finishArea,
  composeReachFinish,
} from "./finish/index.js";
export type {
  FidelityProfile,
  Mesh,
  GeneratedKit,
  GeneratedPiece,
  PieceInstance,
  PieceMeta,
  KitOptions,
  OccupancyGrid,
  OccupancyData,
  DressingAnchor,
  FinishBudgets,
  FinishResult,
  FinishAreaOptions,
} from "./finish/index.js";

// --- volume L3 + content anchors (M06) ---
export {
  sphere,
  ellipsoid,
  box,
  roundBox,
  capsule,
  plane,
  union,
  subtract,
  intersect,
  smoothUnion,
  displace,
  sdfNormal,
  HULL_ARCHETYPES,
  hull,
  catmullRom,
  splineTube,
  connectorTube,
  composeAreaField,
  fieldExtentsFrom,
  forEachCell,
  resolveSocketPose,
  composeAreaVolume,
  composeReachVolume,
} from "./volume/index.js";
export type { Sdf, HullParams, HullArchetype, AreaField, FieldExtents, Coord, SocketBasis, ResolvedPose, AreaVolume, ComposeAreaVolumeParams, ComposeReachVolumeOptions } from "./volume/index.js";
export { quantizeNormal, isQuantized } from "./math/index.js";
export { classifySurface, DEFAULT_ANCHOR_KINDS, scatterSpace, hasLineOfSight } from "./anchors/index.js";
export type { ContentAnchor, ContentAnchorKind, AnchorBinding, RequiredAnchor, ScatterParams } from "./anchors/index.js";
export type { ResolvedSocket } from "./skeleton/index.js";

// --- spatial skeleton L2 (M05) ---
export { forceLayout, splitReachBudget, deriveAreaDials, DEFAULT_AREA_DIALS, envelopeFor, spaceRadius, composeArea, DEFAULT_DEGREE, buildReachSkeleton } from "./skeleton/index.js";
export type {
  LayoutNode,
  LayoutEdge,
  LayoutOptions,
  LayoutResult,
  SpaceBudget,
  AreaDials,
  AreaDialConfig,
  SpaceSpec,
  ProvisionalSocket,
  SocketRef,
  ConnectorKind,
  ConnectorSpec,
  AreaSkeleton,
  ReachSkeleton,
  ComposeAreaParams,
  BuildReachSkeletonOptions,
} from "./skeleton/index.js";

// --- shared type aliases ---
export type { Traversal, SpaceKind, SurfaceKind } from "./types.js";
export type {
  WorldConfig,
  ContentSelector,
  SelectionContext,
  SelectionResult,
  ReachResult,
  ReachMeta,
  ReachEnvelopePreview,
  ComplexityConfig,
  BaselineConfig,
  ReachModifierId,
  ReachModifierDef,
  ReachModifierPolicy,
  DialPatch,
  ReachRequest,
  ReachRequestRecord,
  ReachPortal,
  WorldLengthPolicy,
  AreaCountConfig,
  SchedulableEntry,
} from "./world/index.js";
