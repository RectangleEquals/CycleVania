/**
 * Autosolve — plays the Reach forward like a completionist bot: repeatedly
 * collect every gadget in reachable areas (granting caps) until a fixed point,
 * then step to the terminal if it's reachable. Doubles as the "bot completes a
 * Reach" acceptance harness: for a solvable world it always reaches the terminal.
 */

import { initSim, type SimState } from "./state.js";
import { reachableAreaIds, type SimWorld } from "./world.js";

export function autosolve(world: SimWorld): SimState {
  const s = initSim(world);
  let changed = true;
  while (changed) {
    changed = false;
    const reach = reachableAreaIds(world, s.held);
    for (const aid of reach) {
      s.visited.add(aid);
      const area = world.areas.get(aid);
      for (const g of area?.gadgets ?? []) {
        if (!s.collected.has(g.locationId)) {
          s.collected.add(g.locationId);
          s.inventory.add(g.itemId);
          if (g.cap && !s.held.has(g.cap)) {
            s.held.add(g.cap);
            changed = true;
          }
        }
      }
    }
  }
  const reach = reachableAreaIds(world, s.held);
  if (world.terminalAreaId != null && reach.has(world.terminalAreaId)) {
    s.areaId = world.terminalAreaId;
    s.visited.add(world.terminalAreaId);
    s.log.push(`· reached terminal (area ${world.terminalAreaId})`);
  }
  return s;
}
