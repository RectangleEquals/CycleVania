export { GENERATION_VERSION } from "./version.js";
export { assembleReach, assembleWorld } from "./assemble.js";
export { stableStringify, toTypedKit } from "./serialize.js";
export type { TypedKit, TypedPiece } from "./serialize.js";
export { missionGraphMermaid, reachMissionDiagram, ruleSummary } from "./diagram.js";
export type { DiagramOptions } from "./diagram.js";
export { BUNDLE_KIND, makeBundle, checkReproduction, sniffPayload } from "./bundle.js";
export type { ReproductionBundle, BundleRegistryRef, MakeBundleArgs, ReproCheck, PayloadKind } from "./bundle.js";
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
} from "./shapes.js";
