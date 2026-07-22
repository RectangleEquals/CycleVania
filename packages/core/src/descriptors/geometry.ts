/**
 * Phase D geometry descriptors — the engine-agnostic generated geometry a game
 * textures/renders. CycleVania runs SDF → naturalize → 5° fidelity → dual contouring
 * and emits a modular, world-grid-aligned GeneratedKit (dedup'd unique pieces) +
 * PieceInstances + a serializable occupancy/collision grid + dressing anchors. The
 * piece/kit/instance types live in `geometry/kit.ts`; re-exported here as the output shape.
 */

import type { Vec3 } from "../math/vec.js";
import type { Coord } from "../spatial/grid.js";

export type { GeneratedKit, GeneratedPiece, PieceInstance, PieceMeta, SurfaceKind } from "../geometry/kit.js";
export type { DressingAnchor } from "../geometry/dress.js";

/** Serializable occupancy grid for the descriptor (runtime uses geometry/collision's OccupancyGrid). */
export interface OccupancyData {
  origin: Vec3;
  res: number;
  dims: Coord;
  /** row-major (k*dy+j)*dx+i, 1 = solid rock. */
  solid: number[];
}
