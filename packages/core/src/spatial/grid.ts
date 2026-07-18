/**
 * Grid math — mapping integer cell coordinates to world positions. Used at both
 * levels: the coarse Area node-grid (`areaCellSize`) and the fine Room subdivided
 * grid (`roomCellSize`). Left-handed Z-up: X east, Y north, Z up.
 */

import type { Vec3 } from "../math/vec.js";

export type Coord = readonly [number, number, number];

/** World centre of a cell given the grid origin (min corner) and cell size. */
export function cellCenter(coord: Coord, cellSize: number, origin: Vec3): Vec3 {
  return [
    origin[0] + (coord[0] + 0.5) * cellSize,
    origin[1] + (coord[1] + 0.5) * cellSize,
    origin[2] + (coord[2] + 0.5) * cellSize,
  ];
}

/** World min corner of a cell. */
export function cellMin(coord: Coord, cellSize: number, origin: Vec3): Vec3 {
  return [origin[0] + coord[0] * cellSize, origin[1] + coord[1] * cellSize, origin[2] + coord[2] * cellSize];
}

export const coordKey = (c: Coord): string => `${c[0]},${c[1]},${c[2]}`;
