import type { RoomKind } from "../types.js";

/**
 * Room archetype — a family of room shapes/behaviors the AreaComposer can choose
 * (weighted by style). Sizes are in fine cells; the AreaComposer clamps to the
 * complexity budget and hands the RoomComposer an extent envelope.
 */
export interface RoomArchetype {
  id: string;
  kinds: RoomKind[];
  /** Min/max extent in fine cells (per axis, roughly). */
  sizeRange: [number, number];
  tags: string[];
}
