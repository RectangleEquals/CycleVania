/**
 * Puzzles & Locks — a first-class, schedulable content pool with the identical
 * machinery Capabilities get. A `PuzzleDef`'s `condition` is the SAME Rule algebra
 * as every edge, so composing a puzzle with a gadget lock is free. A `PuzzleDef`
 * is a `SchedulableEntry` (id + powerWeight + guarantee) directly.
 */

import type { Rule, CapabilityId } from "../logic/index.js";

export type PuzzleScope = "room" | "area" | "reach" | "world";
export type PuzzleClass = "required" | "optional-reward" | "optional-shortcut" | "cosmetic";

export interface EdgeSpec {
  from?: string; // region id; defaults to the bound Region
  to: string;
  oneWay?: boolean;
}

export interface ItemSpec {
  itemId: string;
}

export type PuzzleOutcome =
  | { kind: "open-edge"; edge: EdgeSpec }
  | { kind: "spawn-item-here"; item: ItemSpec }
  | { kind: "unlock-adjacent-space"; spaceHint: string }
  | { kind: "grant-capability"; capability: CapabilityId }
  | { kind: "set-flag-only" }
  | { kind: "world-ending" };

export interface PuzzleDef {
  id: string;
  scope: PuzzleScope;
  class: PuzzleClass;
  condition: Rule;
  outcome: PuzzleOutcome;
  powerWeight: (level: number) => number;
  guarantee?: { withinReachLevels: number };
  spatialRecipe?: string;
  archetype?: string;
  revision?: number;
}

/** A placed puzzle. `condition` is shared BY REFERENCE with the bound edge. */
export interface PuzzleInstance {
  instanceId: string;
  defId: string;
  scope: PuzzleScope;
  class: PuzzleClass;
  condition: Rule;
  outcome: PuzzleOutcome;
  spatialRecipe?: string;
}

export interface PuzzleEconomyConfig {
  min: number;
  max: number;
}

export const DEFAULT_PUZZLE_ECONOMY: PuzzleEconomyConfig = { min: 1, max: 3 };
