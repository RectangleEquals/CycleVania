import type { SnapPolicy } from "../types.js";

/**
 * Grid resolutions the parent project must set. `areaCellSize` is the coarse grid
 * room-nodes are placed on; `roomCellSize` is the fine subdivision each room's
 * cells occupy. `snap` constrains geometry angles (PS2 palette by default).
 */
export interface GridConfig {
  areaCellSize: number;
  roomCellSize: number;
  snap: SnapPolicy;
}
