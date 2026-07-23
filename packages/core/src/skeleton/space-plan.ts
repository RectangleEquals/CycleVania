/**
 * L2 shapes — abstract Space facts (all decided here, including outdoor + landmark),
 * provisional sockets (positions on envelope boundaries; resolved onto hull
 * surfaces in L3), and connector plans (with per-traversal length bounds). L1
 * gates ride onto sockets/connectors BY REFERENCE (never re-derived).
 */

import { boxFromCenterHalf, type Vec3, type WorldBox } from "../math/index.js";
import type { NodeRole, RegionId } from "../graph/index.js";
import type { Rule } from "../logic/index.js";
import type { Traversal } from "../types.js";
import type { SpaceBudget } from "./space-budget.js";

export interface SpaceSpec {
  id: string;
  kind: "room" | "outdoor";
  role: NodeRole | "junction";
  regionId: RegionId;
  budget: SpaceBudget;
  origin: Vec3;
  envelope: WorldBox;
  biome: string;
  sub?: { biome: string; blend: number };
  outdoor: boolean;
  landmark: boolean;
  reservedRecipes: string[];
  hidden: boolean; // secret-biased Space
}

export interface ProvisionalSocket {
  id: string;
  spaceId: string;
  pos: Vec3;
  dir: Vec3;
  kind: "structural";
  traversal: Traversal;
  signature: string;
  gate?: Rule;
  oneWay?: boolean;
  partner?: { spaceId: string; socketId: string };
}

export interface SocketRef {
  spaceId: string;
  socketId: string;
}

export type ConnectorKind = "straight" | "curved" | "ramp" | "shaft" | "crawl" | "open-seam";

export interface ConnectorSpec {
  id: string;
  from: SocketRef;
  to: SocketRef;
  kind: ConnectorKind;
  traversal: Traversal;
  lengthBounds: { min: number; max: number };
  gate?: Rule;
  oneWay?: boolean;
  waypoints?: Vec3[];
}

/** A socket resolved onto the hull surface with a fidelity-snapped basis (L3). */
export interface ResolvedSocket {
  id: string;
  spaceId: string;
  pos: Vec3;
  basis: { forward: Vec3; up: Vec3; right: Vec3 };
  radius: number;
  kind: "structural";
  traversal: Traversal;
  signature: string;
  gate?: Rule;
  oneWay?: boolean;
  partner?: { spaceId: string; socketId: string };
  passable: boolean;
}

export interface AreaSkeleton {
  regionId: RegionId;
  role: NodeRole;
  spaces: SpaceSpec[];
  connectors: ConnectorSpec[]; // intra-area
  sockets: ProvisionalSocket[]; // every provisional socket (intra + boundary)
  boundarySockets: ProvisionalSocket[]; // subset for inter-area connections
  entrySpaceId: string;
  relaxations: string[];
  // --- L3 (M06) additions, filled by the volume pass ---
  resolvedSockets?: ResolvedSocket[];
  anchors?: import("../anchors/index.js").ContentAnchor[];
  /** Non-serializable: the composed SDF field (generation-only; consumed by L4). */
  field?: import("../volume/field.js").AreaField;
  // --- L4 (M07) addition, filled by the finish pass when geometry is enabled ---
  finish?: import("../finish/index.js").FinishResult;
}

export interface ReachSkeleton {
  areas: AreaSkeleton[];
  areaOrigins: Record<RegionId, Vec3>;
  interAreaConnectors: ConnectorSpec[];
  landmarkSpaceIds: string[];
  relaxations: string[];
}

/** Half-extent of an envelope from a volume budget (roughly cubic, biased flat). */
export function envelopeFor(origin: Vec3, volumeCells: number): WorldBox {
  const s = Math.max(2, Math.cbrt(Math.max(1, volumeCells)));
  return boxFromCenterHalf(origin, [s, s, Math.max(1.5, s * 0.7)]);
}

/** Bounding radius used by the force layout (from the envelope's XY extent). */
export function spaceRadius(volumeCells: number): number {
  return Math.max(2, Math.cbrt(Math.max(1, volumeCells)) * 1.15);
}
