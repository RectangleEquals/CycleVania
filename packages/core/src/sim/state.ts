/** Simulator state — everything a playthrough tracks. Cloned in/out of `step`. */

import { CapSet, type CapabilityId } from "../logic/index.js";
import type { RegionId, LocationId } from "../graph/index.js";
import type { SimWorld } from "./world.js";

export interface SimState {
  at: RegionId;
  held: CapSet;
  inventory: Set<string>;
  collected: Set<LocationId>;
  solvedPuzzles: Set<string>;
  openedOneWays: Set<string>;
  visited: Set<RegionId>;
  /** volatile flag name → the region that set it (cleared on leaving that region). */
  volatileFlags: Map<string, RegionId>;
  log: string[];
}

export function initSim(world: SimWorld): SimState {
  return {
    at: world.start,
    held: world.startHeld.clone(),
    inventory: new Set(),
    collected: new Set(),
    solvedPuzzles: new Set(),
    openedOneWays: new Set(),
    visited: new Set([world.start]),
    volatileFlags: new Map(),
    log: [],
  };
}

export function cloneState(s: SimState): SimState {
  return {
    at: s.at,
    held: s.held.clone(),
    inventory: new Set(s.inventory),
    collected: new Set(s.collected),
    solvedPuzzles: new Set(s.solvedPuzzles),
    openedOneWays: new Set(s.openedOneWays),
    visited: new Set(s.visited),
    volatileFlags: new Map(s.volatileFlags),
    log: [...s.log],
  };
}

export type { CapabilityId };
