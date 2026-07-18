/**
 * Abstract, renderer-free output. CycleVania emits kit-piece assignments per
 * cell + socket transforms + connector plans; the PARENT stitches and renders.
 * No meshes, no materials, no engine types ever appear here.
 */

import type { Vec3, WorldBox } from "../math/vec.js";
import type { Capability, Rule } from "../logic/index.js";
import type { ConnectorKind, RoomKind, Traversal } from "../types.js";
import type { Socket } from "../spatial/sockets.js";
import type { Coord } from "../spatial/grid.js";

/** Something placed into a cell that isn't structural geometry. */
export interface ContentAnchor {
  kind: string; // "gadget" | "pickup" | "enemy" | "switch" | "puzzle" | "prop" | …
  pos: Vec3;
  /** locationId / itemId / lock name, depending on kind. */
  ref?: string;
}

/** A placed progression item (resolved from the solver's placement). */
export interface GadgetPlacement {
  itemId: string;
  cap?: Capability;
  locationId: string;
  pos: Vec3;
}

/** One subdivided room cell: a kit-piece assignment + metadata. */
export interface CellDescriptor {
  coord: Coord;
  role: string; // cell surface role (floor/ceiling/wall/corner/opening/air)
  /** GeometryKit piece id, or null for empty air. */
  kitId: string | null;
  yaw: number; // snapped orientation (radians)
  traversal?: Traversal;
  sockets: Socket[];
  contents: ContentAnchor[];
}

/** A room = a subdivided cluster of cells. */
export interface RoomDescriptor {
  nodeId: string;
  regionId: string;
  role: string; // region role that owns this room
  kind: RoomKind;
  origin: Vec3; // world min corner
  footprint: Coord; // extent in fine cells
  cells: CellDescriptor[];
  sockets: Socket[];
  bounds: WorldBox;
  styleId: string;
  seed: string;
}

/** A connector joins two sockets — any non-boxy geometry. */
export interface ConnectorPlan {
  id: string;
  fromRoom: string;
  toRoom: string;
  fromSocket: string;
  toSocket: string;
  kind: ConnectorKind;
  cells: CellDescriptor[];
  requires?: Rule;
  /** Full gate cap-set (never collapsed to one "primary"). */
  requiredCaps?: Capability[];
  oneWay?: boolean;
}

/** An inter-area portal (keyed to a region edge). */
export interface PortalSpec {
  key: string;
  edge?: { from: string; to: string };
  trigger: WorldBox;
  spawn: Vec3;
  spawnYaw: number;
  requires?: Rule;
  requiredCaps?: Capability[];
  oneWay?: boolean;
}

/** An area = a 3D grid of room-nodes joined by connectors. */
export interface AreaDescriptor {
  areaId: number;
  regionId: string;
  role: string;
  nodeGrid: { res: number; dims: Coord };
  rooms: RoomDescriptor[];
  connectors: ConnectorPlan[];
  portals: PortalSpec[];
  gadgets: GadgetPlacement[];
  bounds: WorldBox;
  styleId: string;
  seed: string;
}

/** A directed link between two areas (carries the region edge's gate). */
export interface AreaLink {
  fromAreaId: number;
  toAreaId: number;
  requires: Rule;
  requiredCaps: Capability[];
  oneWay?: boolean;
}

/** A Reach = an array of areas + their connections. */
export interface ReachDescriptor {
  areas: AreaDescriptor[];
  links: AreaLink[];
  startAreaId: number;
  bounds: WorldBox;
}
