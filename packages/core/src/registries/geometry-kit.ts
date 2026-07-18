/**
 * GeometryKit — the parent's modular kit as a grammar. It maps logically-named
 * surface roles (FlatLeftWall, TopLeftCorner, Ramp45, CurvedHall, OpenEdge,
 * Crawlspace, ClimbFace, DropHatch, …) to available pieces + metadata. The
 * RoomComposer selects one piece per cell consistent with the cell's role,
 * adjacency grammar, snap policy and availability. NO geometry lives here — the
 * parent realizes meshes from the kit id.
 */

import type { CellFace, Traversal } from "../types.js";

export interface AdjacencyRule {
  /** Piece roles that may not sit directly beside this one. */
  forbidBeside?: string[];
  /** Piece roles that must sit directly below this one (e.g. a ledge needs support). */
  requireBelow?: string[];
}

export interface KitPiece {
  id: string;
  /** Logical surface role this piece fills. */
  role: string;
  /** Which cell faces / interior the piece occupies. */
  occupies?: CellFace[];
  /** Allowed rotations (PS2 palette by default: e.g. multiples of 45°). */
  snapAngles: number[];
  /** For socketable pieces: how the aperture is traversed. */
  traversal?: Traversal;
  socketCapable?: boolean;
  tags: string[];
  styleId?: string;
  collider?: "solid" | "none" | "trigger";
  adjacency?: AdjacencyRule;
}

export interface GeometryKit {
  pieces: KitPiece[];
}

/** All pieces registered for a given surface role. */
export function piecesForRole(kit: GeometryKit, role: string): KitPiece[] {
  return kit.pieces.filter((p) => p.role === role);
}

/** Roles this kit provides at least one piece for. */
export function kitRoles(kit: GeometryKit): Set<string> {
  return new Set(kit.pieces.map((p) => p.role));
}
