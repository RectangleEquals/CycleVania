/**
 * Cell classification — given a cell's position inside a room's extent, decide
 * its logical surface role and which faces are on the room boundary. The
 * RoomComposer maps the role to a GeometryKit piece. This is deliberately a
 * hollow-box model for the Phase A slice; richer classifiers (snake/non-convex,
 * multi-level, curved) plug in behind the same interface.
 */

import type { CellFace } from "../types.js";
import type { Coord } from "./grid.js";

export type CellRole = "floor" | "ceiling" | "wall" | "corner" | "opening" | "air";

export interface CellClass {
  role: CellRole;
  faces: CellFace[];
}

/** Boundary faces of a cell within an [ex,ey,ez] extent. */
export function boundaryFaces(coord: Coord, extent: Coord): CellFace[] {
  const faces: CellFace[] = [];
  if (coord[0] === 0) faces.push("nx");
  if (coord[0] === extent[0] - 1) faces.push("px");
  if (coord[1] === 0) faces.push("ny");
  if (coord[1] === extent[1] - 1) faces.push("py");
  if (coord[2] === 0) faces.push("nz");
  if (coord[2] === extent[2] - 1) faces.push("pz");
  return faces;
}

/** Classify a cell in a hollow-box room: floor slab, ceiling, perimeter walls/corners, else air. */
export function classifyCell(coord: Coord, extent: Coord): CellClass {
  const [x, y, z] = coord;
  const [ex, ey, ez] = extent;
  const faces = boundaryFaces(coord, extent);
  if (z === 0) return { role: "floor", faces };
  if (ez > 1 && z === ez - 1) return { role: "ceiling", faces };

  const onX = x === 0 || x === ex - 1;
  const onY = y === 0 || y === ey - 1;
  if (onX && onY) return { role: "corner", faces };
  if (onX || onY) return { role: "wall", faces };
  return { role: "air", faces };
}
