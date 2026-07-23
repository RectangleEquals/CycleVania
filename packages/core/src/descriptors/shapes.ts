/**
 * The output contract — plain, serializable, self-describing data. `number[]`
 * buffers, string ids, no engine types, no functions. Optional fields are ABSENT,
 * never null. Many shapes reuse already-JSON-safe internal types (Rule, ContentAnchor,
 * PieceInstance, GeneratedKit, OccupancyData, DressingAnchor, ReachPortal).
 */

import type { Rule, HeldData } from "../logic/index.js";
import type { MissionGraph } from "../graph/index.js";
import type { PuzzleInstance } from "../puzzle/index.js";
import type { ContentAnchor } from "../anchors/index.js";
import type { GeneratedKit, PieceInstance, OccupancyData, DressingAnchor } from "../finish/index.js";
import type { ReachPortal, ReachRequestRecord } from "../world/index.js";

export type Vec3Data = [number, number, number];
export interface WorldBoxData {
  min: Vec3Data;
  max: Vec3Data;
}

export interface SocketData {
  id: string;
  spaceId: string;
  pos: Vec3Data;
  basis?: { forward: Vec3Data; up: Vec3Data; right: Vec3Data };
  radius?: number;
  kind: string;
  traversal: string;
  signature: string;
  gate?: Rule;
  oneWay?: boolean;
  partner?: { spaceId: string; socketId: string };
  passable?: boolean;
}

export interface ConnectorDescriptor {
  id: string;
  from: { spaceId: string; socketId: string };
  to: { spaceId: string; socketId: string };
  kind: string;
  traversal: string;
  gate?: Rule;
  oneWay?: boolean;
  waypoints?: Vec3Data[];
}

export interface SpaceDescriptor {
  id: string;
  kind: string;
  role: string;
  regionId: string;
  origin: Vec3Data;
  bounds: WorldBoxData;
  outdoor: boolean;
  landmark: boolean;
  hidden: boolean;
  biome: string;
  subBiome?: { id: string; blend: number };
  recipeIds: string[];
  sockets: SocketData[];
}

export interface AreaDescriptor {
  regionId: string;
  role: string;
  biome: string;
  bounds: WorldBoxData;
  spaces: SpaceDescriptor[];
  connectors: ConnectorDescriptor[];
  anchors: ContentAnchor[];
  kit?: GeneratedKit;
  instances?: PieceInstance[];
  occupancy?: OccupancyData;
  dressing?: DressingAnchor[];
}

export interface MissionGraphData {
  regions: { id: string; role: string }[];
  edges: { from: string; to: string; rule: Rule; oneWay?: boolean }[];
  flags: { name: string; setBy: string; volatile?: boolean }[];
  locations: { id: string; region: string; gate?: Rule; bonus?: boolean }[];
  start: string;
}

export interface PlacementEntry {
  locationId: string;
  itemId: string;
}

export interface ReachMetaData {
  reachIndex: number;
  requestIdentity: string;
  chosenModifiers: string[];
  finalCeiling: number;
  areaCount: number;
  buckets: Record<string, number>;
  spheres: string[][];
  relaxations: string[];
  startHeld: HeldData;
}

export interface ReachDescriptor {
  meta: ReachMetaData;
  graph: MissionGraphData;
  placement: PlacementEntry[];
  puzzles: PuzzleInstance[];
  areas: AreaDescriptor[];
  interAreaConnectors: ConnectorDescriptor[];
}

export interface WorldMetaData {
  worldSeed: string;
  generationVersion: string;
  registryFingerprint: string;
  lengthPolicy?: { min: number; max?: number };
  drawnLength?: number;
  requestLog: ReachRequestRecord[];
}

export interface WorldDescriptor {
  meta: WorldMetaData;
  reachPortals: ReachPortal[];
  reaches: ReachDescriptor[];
}

export type { MissionGraph };
