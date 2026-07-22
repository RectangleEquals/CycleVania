/**
 * Sockets — 3D directional connection points. A socket is not always a door: it
 * can be a climb-face, a crawlspace, a drop-hatch, a vent, or an open outdoor
 * seam, at any altitude/angle, with a traversal mode and an optional gate. This
 * is the contract connectors join room-to-room.
 */

import type { Vec3 } from "../math/vec.js";
import { cross, normalize } from "../math/vec.js";
import type { Rule } from "../logic/index.js";
import type { CellFace, SocketKind, Traversal } from "../types.js";
import type { Coord } from "./grid.js";

/** An orthonormal aperture frame (organic sockets angle/curve/exit ceilings). */
export interface SocketBasis {
  right: Vec3;
  up: Vec3;
  forward: Vec3; // == Socket.dir (outward)
}

/** Extra generation semantics an organic socket carries (Gemini L3). */
export interface SocketMeta {
  biome?: string;
  size?: string; // "small" | "wide" | "grand" | …
  type?: string; // "MorphBall_Slot" | "cliff-seam" | …
}

export interface Socket {
  id: string;
  /** The cell (room-local coord) that hosts the aperture. */
  cell: Coord;
  face: CellFace;
  /** Doorway centre in world space. */
  pos: Vec3;
  /** Outward unit normal (may point ±Z for vertical traversal). */
  dir: Vec3;
  width: number;
  kind: SocketKind;
  traversal: Traversal;
  /** Capability rule required to traverse (climb→some cap, crawl→small-form, …). */
  gate?: Rule;
  /** One-way aperture (drop): forward only. */
  oneWay?: boolean;
  /** Full organic transform frame (arbitrary angle/curve, 5°-snapped by the composer). */
  basis?: SocketBasis;
  /** Aperture radius (for spline-connector carve + hull socket carve). */
  radius?: number;
  /** Generation metadata (biome/size/type). */
  meta?: SocketMeta;
}

/** Build an orthonormal aperture frame from an outward direction (+ optional up hint). */
export function socketBasis(dir: Vec3, upHint: Vec3 = [0, 0, 1]): SocketBasis {
  const forward = normalize(dir);
  let up = upHint;
  if (Math.abs(forward[0] * up[0] + forward[1] * up[1] + forward[2] * up[2]) > 0.95) up = [1, 0, 0]; // avoid parallel
  const right = normalize(cross(forward, up));
  const trueUp = normalize(cross(right, forward));
  return { right, up: trueUp, forward };
}

/** Outward unit normal for a boundary face. */
export function faceNormal(face: CellFace): Vec3 {
  switch (face) {
    case "px":
      return [1, 0, 0];
    case "nx":
      return [-1, 0, 0];
    case "py":
      return [0, 1, 0];
    case "ny":
      return [0, -1, 0];
    case "pz":
      return [0, 0, 1];
    case "nz":
      return [0, 0, -1];
    case "interior":
      return [0, 0, 0];
  }
}

/** The opposite face — used to line up a connector's two ends. */
export function oppositeFace(face: CellFace): CellFace {
  const map: Record<CellFace, CellFace> = {
    px: "nx",
    nx: "px",
    py: "ny",
    ny: "py",
    pz: "nz",
    nz: "pz",
    interior: "interior",
  };
  return map[face];
}
