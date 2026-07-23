/**
 * Spheres — the solvability ladder. Sphere 0 = reachable from `start` with
 * `startHeld`; sphere n+1 = reachable once everything in spheres ≤ n is
 * collected. Playing forward, collect every reachable Location, folding placed
 * items' grants and non-volatile flag-setters into the held state until a fixed
 * point. Volatile flags are deliberately NOT auto-set here — the baseline proof
 * runs against non-volatile state only.
 */

import { CapSet, type HeldData } from "../logic/index.js";
import { reachableLocations } from "./reachability.js";
import type { Item, LocationId, MissionGraph, Placement } from "./mission-graph.js";

export interface SphereResult {
  /** location ids first reachable at each sphere, in order. */
  spheres: LocationId[][];
  /** held snapshot after each sphere. */
  heldPerSphere: HeldData[];
  /** everything collectible playing forward. */
  finalHeld: CapSet;
  /** every NON-bonus Location is reachable (no stranded required slots). */
  reachedAll: boolean;
}

export function computeSpheres(
  g: MissionGraph,
  startHeld: CapSet,
  placement: Placement,
  items: readonly Item[],
): SphereResult {
  const itemsById = new Map(items.map((i) => [i.id, i] as const));

  // Non-volatile flag setters: reaching the setter Location sets the flag.
  const setters = new Map<LocationId, string[]>();
  for (const f of g.flags) {
    if (f.volatile) continue;
    const list = setters.get(f.setBy);
    if (list) list.push(f.name);
    else setters.set(f.setBy, [f.name]);
  }

  const held = startHeld.clone();
  const collected = new Set<LocationId>();
  const spheres: LocationId[][] = [];
  const heldPerSphere: HeldData[] = [];

  for (;;) {
    const newly = reachableLocations(g, held).filter((l) => !collected.has(l));
    if (newly.length === 0) break;
    spheres.push(newly);
    for (const l of newly) {
      collected.add(l);
      const itemId = placement.get(l);
      const item = itemId !== undefined ? itemsById.get(itemId) : undefined;
      if (item?.grants) for (const cap of item.grants) held.add(cap, 1);
      const flags = setters.get(l);
      if (flags) for (const f of flags) held.addFlag(f);
    }
    heldPerSphere.push(held.toData());
  }

  let reachedAll = true;
  for (const loc of g.locations) {
    if (!loc.bonus && !collected.has(loc.id)) {
      reachedAll = false;
      break;
    }
  }

  return { spheres, heldPerSphere, finalHeld: held, reachedAll };
}
