/**
 * Simulator state — a deterministic snapshot of a playthrough. `held` carries
 * capabilities/counts/flags; `inventory` tracks collected item ids; `collected`
 * tracks collected location ids; `visited` tracks areas seen. The reducer clones
 * this each step so `step` is a pure function.
 */

import { CapSet, heldOf } from "../logic/index.js";
import type { SimWorld } from "./world.js";

export interface SimState {
  areaId: number;
  held: CapSet;
  inventory: Set<string>;
  collected: Set<string>;
  visited: Set<number>;
  log: string[];
}

export function initSim(world: SimWorld): SimState {
  return {
    areaId: world.startAreaId,
    held: heldOf(world.startCaps),
    inventory: new Set(),
    collected: new Set(),
    visited: new Set([world.startAreaId]),
    log: [],
  };
}

export function cloneState(s: SimState): SimState {
  return {
    areaId: s.areaId,
    held: s.held.clone(),
    inventory: new Set(s.inventory),
    collected: new Set(s.collected),
    visited: new Set(s.visited),
    log: [...s.log],
  };
}
