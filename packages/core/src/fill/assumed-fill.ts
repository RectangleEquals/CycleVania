/**
 * Assumed fill — solvability constructed, not checked. To place an item, assume
 * the party holds every OTHER unplaced item's grants (plus startHeld); the
 * candidate Locations are those currently reachable, empty, and non-bonus; drop
 * the item into one by a seeded, exploration-weighted draw. Inductively an item is
 * only ever gated behind items placed AFTER it → a valid sphere ordering, so a
 * softlock is structurally impossible. No retry loop: the bootstrap invariant
 * guarantees a candidate always exists; preference weights relax (recorded, never
 * silent) only if a step's candidates all zero out.
 *
 * Counted keys fall out free: N copies of one capability, "assume all others" gives
 * count N-1 while placing the Nth, so a count(cap,N) gate can't hide its last copy.
 */

import { CapSet } from "../logic/index.js";
import type { Rng } from "../math/index.js";
import { isSolvable, reachableRegions, reachableLocations } from "../graph/index.js";
import type { Item, LocationId, MissionGraph, Placement, RegionId } from "../graph/index.js";
import {
  DEFAULT_PLACEMENT_WEIGHTS,
  locationWeight,
  type CandidateLocation,
  type PlacementWeightConfig,
  type WeightContext,
} from "./placement-weights.js";

export interface FillResult {
  placement: Placement;
  relaxations: string[];
}

interface RegionMeta {
  depthRank: number;
  behindGateCount: number;
}

/** Structural BFS depth + minimum gated-edge count from start to each region. */
function regionMetrics(g: MissionGraph): Map<RegionId, RegionMeta> {
  const depth = new Map<RegionId, number>([[g.start, 0]]);
  const gates = new Map<RegionId, number>([[g.start, 0]]);
  // 0/1-weight relaxation to a fixed point (graphs are small).
  let changed = true;
  while (changed) {
    changed = false;
    for (const e of g.edges) {
      const dFrom = depth.get(e.from);
      if (dFrom === undefined) continue;
      const gFrom = gates.get(e.from) ?? 0;
      const gCost = e.rule.k === "always" ? 0 : 1;
      const dTo = depth.get(e.to);
      if (dTo === undefined || dFrom + 1 < dTo) {
        depth.set(e.to, dFrom + 1);
        changed = true;
      }
      const gTo = gates.get(e.to);
      if (gTo === undefined || gFrom + gCost < gTo) {
        gates.set(e.to, gFrom + gCost);
        changed = true;
      }
    }
  }
  const out = new Map<RegionId, RegionMeta>();
  for (const r of g.regions) {
    out.set(r.id, { depthRank: depth.get(r.id) ?? 0, behindGateCount: gates.get(r.id) ?? 0 });
  }
  return out;
}

function recordOnce(list: string[], code: string): void {
  if (!list.includes(code)) list.push(code);
}

export function assumedFill(
  g: MissionGraph,
  startHeld: CapSet,
  items: readonly Item[],
  rng: Rng,
  weights: PlacementWeightConfig = DEFAULT_PLACEMENT_WEIGHTS,
): FillResult {
  const relaxations: string[] = [];
  const progression = items.filter((i) => i.class === "progression");
  const nonProgression = items.filter((i) => i.class !== "progression");

  const roleOf = new Map(g.regions.map((r) => [r.id, r.role] as const));
  const regionOfLoc = new Map(g.locations.map((l) => [l.id, l.region] as const));
  const bonusLoc = new Set(g.locations.filter((l) => l.bonus).map((l) => l.id));
  const metrics = regionMetrics(g);

  const placement: Placement = new Map();
  const placedInRegion = new Map<RegionId, number>();

  // Assume every progression grant is held, then subtract each as it is placed.
  const assumed = startHeld.clone();
  for (const it of progression) for (const cap of it.grants ?? []) assumed.add(cap, 1);

  let lastDepthHint: number | undefined;

  for (const item of progression) {
    for (const cap of item.grants ?? []) assumed.add(cap, -1); // stop assuming the one being placed

    const candidates = reachableLocations(g, assumed).filter((l) => !placement.has(l) && !bonusLoc.has(l));
    if (candidates.length === 0) {
      throw new Error(
        `assumedFill: no reachable Location for item "${item.id}" — bootstrap invariant violated ` +
          `(a template must over-provision always-reachable hub slots).`,
      );
    }

    const toCandidate = (l: LocationId): CandidateLocation => {
      const regionId = regionOfLoc.get(l) as RegionId;
      const m = metrics.get(regionId) ?? { depthRank: 0, behindGateCount: 0 };
      return {
        locationId: l,
        regionId,
        regionRole: roleOf.get(regionId) ?? "segment",
        depthRank: m.depthRank,
        behindGateCount: m.behindGateCount,
        isEntry: regionId === g.start,
      };
    };
    const infos = candidates.map(toCandidate);
    const ctx: WeightContext =
      lastDepthHint !== undefined ? { placedInRegion, lastDepthHint } : { placedInRegion };

    // Weighted draw, relaxing preferences (recorded) only if all weights zero out.
    let cfg = weights;
    let entries = infos.map((c) => ({ item: c.locationId, weight: locationWeight(c, ctx, cfg) }));
    if (entries.every((e) => e.weight <= 0)) {
      recordOnce(relaxations, "fill.relaxed-per-region-cap");
      cfg = { ...cfg, perRegionCap: Number.POSITIVE_INFINITY };
      entries = infos.map((c) => ({ item: c.locationId, weight: locationWeight(c, ctx, cfg) }));
    }
    if (entries.every((e) => e.weight <= 0)) {
      recordOnce(relaxations, "fill.relaxed-entry-space");
      cfg = { ...cfg, entrySpaceWeight: cfg.entrySpaceWeight > 0 ? cfg.entrySpaceWeight : 1 };
      entries = infos.map((c) => ({ item: c.locationId, weight: locationWeight(c, ctx, cfg) }));
    }
    if (entries.every((e) => e.weight <= 0)) entries = infos.map((c) => ({ item: c.locationId, weight: 1 }));

    const chosen = rng.weighted(entries);
    placement.set(chosen, item.id);
    const chosenRegion = regionOfLoc.get(chosen) as RegionId;
    placedInRegion.set(chosenRegion, (placedInRegion.get(chosenRegion) ?? 0) + 1);
    lastDepthHint = metrics.get(chosenRegion)?.depthRank;
  }

  // Fill remaining Locations with non-progression items (bonus-gated slots allowed).
  fillRemaining(g, placement, nonProgression, rng);

  if (!isSolvable(g, startHeld, items, placement)) {
    throw new Error("assumedFill: constructed placement is not solvable — internal invariant violated.");
  }
  return { placement, relaxations };
}

/** Fill every empty Location with a non-progression item id, if any exist. */
export function fillRemaining(g: MissionGraph, placement: Placement, pool: readonly Item[], rng: Rng): Placement {
  if (pool.length === 0) return placement;
  const ids = pool.map((i) => i.id);
  for (const loc of g.locations) {
    if (!placement.has(loc.id)) placement.set(loc.id, ids[rng.int(0, ids.length - 1)] as string);
  }
  return placement;
}
