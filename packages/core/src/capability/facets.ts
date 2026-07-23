/**
 * Facet evaluation — resolve derived (combo) held-state, aggregate Magnitude
 * Facets per budget bucket (the world-shaping feedback loop), and collect active
 * Tags. All host callbacks are called here with the thin, game-agnostic
 * `FacetContext`; iteration order is fixed (catalog order) for determinism.
 */

import { CapSet, type CapabilityId } from "../logic/index.js";
import type { CapabilityDef, FacetContext } from "./capability-def.js";

/**
 * Build a `CapSet` from raw grants + flags, resolving derived capabilities to a
 * fixed point (a derived cap is held once its `derivedFrom` are held at
 * `minLevels`).
 */
export function buildHeld(
  defs: ReadonlyMap<CapabilityId, CapabilityDef>,
  grants: ReadonlyMap<CapabilityId, number>,
  flags: Iterable<string> = [],
): CapSet {
  const held = new CapSet();
  for (const [id, n] of grants) if (n > 0) held.add(id, n);
  for (const f of flags) held.addFlag(f);
  let changed = true;
  while (changed) {
    changed = false;
    for (const def of defs.values()) {
      if (def.held === "granted" || held.hasCap(def.id)) continue;
      const d = def.held;
      const ok = d.derivedFrom.every((pre) => held.capCount(pre) >= (d.minLevels?.[pre] ?? 1));
      if (ok) {
        held.add(def.id, 1);
        changed = true;
      }
    }
  }
  return held;
}

function facetContext(level: number, heldSet: ReadonlySet<CapabilityId>, def: CapabilityDef): FacetContext {
  // Conservative resource assumption: charge == capacity (sustained value).
  const rf = def.facets.find((f) => f.kind === "resource");
  if (rf && rf.kind === "resource") {
    const cap = rf.capacity({ level, held: heldSet });
    return { level, held: heldSet, resource: { charge: cap, capacity: cap } };
  }
  return { level, held: heldSet };
}

/** Sum every held capability's Magnitude Facets per bucket (fixed catalog order). */
export function aggregateBuckets(defs: ReadonlyMap<CapabilityId, CapabilityDef>, held: CapSet): Record<string, number> {
  const heldSet = new Set(held.capIds());
  const buckets: Record<string, number> = {};
  for (const def of defs.values()) {
    if (!heldSet.has(def.id)) continue;
    const level = held.capCount(def.id);
    for (const f of def.facets) {
      if (f.kind !== "magnitude") continue;
      const v = f.evaluate(facetContext(level, heldSet, def));
      buckets[f.bucket] = (buckets[f.bucket] ?? 0) + v;
    }
  }
  return buckets;
}

/** Tags active given the held state (a TagFacet with no `evaluate` is always active). */
export function activeTags(defs: ReadonlyMap<CapabilityId, CapabilityDef>, held: CapSet): Set<string> {
  const heldSet = new Set(held.capIds());
  const tags = new Set<string>();
  for (const def of defs.values()) {
    if (!heldSet.has(def.id)) continue;
    const level = held.capCount(def.id);
    for (const f of def.facets) {
      if (f.kind !== "tag") continue;
      if (f.evaluate === undefined || f.evaluate(facetContext(level, heldSet, def))) tags.add(f.tag);
    }
  }
  return tags;
}
