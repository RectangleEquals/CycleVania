/**
 * Occupancy — tracks which coarse Area cells are taken so room-nodes and
 * connectors don't overlap. The AreaComposer uses it to place nodes and to grow
 * rooms into leftover space.
 */

import type { Coord } from "./grid.js";
import { coordKey } from "./grid.js";

export class Occupancy {
  private taken = new Set<string>();

  occupy(c: Coord): void {
    this.taken.add(coordKey(c));
  }

  isFree(c: Coord): boolean {
    return !this.taken.has(coordKey(c));
  }

  /** Occupy an inclusive box of coarse cells. */
  occupyBox(min: Coord, max: Coord): void {
    for (let x = min[0]; x <= max[0]; x++)
      for (let y = min[1]; y <= max[1]; y++)
        for (let z = min[2]; z <= max[2]; z++) this.occupy([x, y, z]);
  }

  /** Is every cell of an inclusive box free? */
  boxFree(min: Coord, max: Coord): boolean {
    for (let x = min[0]; x <= max[0]; x++)
      for (let y = min[1]; y <= max[1]; y++)
        for (let z = min[2]; z <= max[2]; z++) if (!this.isFree([x, y, z])) return false;
    return true;
  }

  get size(): number {
    return this.taken.size;
  }
}
