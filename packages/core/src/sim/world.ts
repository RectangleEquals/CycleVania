/**
 * SimWorld — a playtest view of a realized Reach over its MISSION GRAPH (regions +
 * gated links + placed items + puzzles). The reducer and autosolver walk it the way
 * a player would: forward links gated by their rule, walking back open unless
 * one-way. Operates on ABSTRACT facts only (no geometry). Built from a ReachResult.
 */

import { evalRule, heldFromData, CapSet, type CapabilityId, type Held, type Rule } from "../logic/index.js";
import type { NodeRole, RegionId, LocationId } from "../graph/index.js";
import type { PuzzleInstance } from "../puzzle/index.js";
import type { ReachResult } from "../world/index.js";

export interface SimItemInfo {
  id: string;
  class: string;
  grants: CapabilityId[];
}

export interface SimLocation {
  id: LocationId;
  itemId?: string;
  bonus?: boolean;
  sphere?: number;
}

export interface SimNode {
  id: RegionId;
  role: NodeRole;
  locations: SimLocation[];
}

export interface SimLink {
  from: RegionId;
  to: RegionId;
  rule: Rule;
  oneWay?: boolean;
}

export interface SimWorld {
  start: RegionId;
  terminal: RegionId | undefined;
  startHeld: CapSet;
  nodes: Map<RegionId, SimNode>;
  links: SimLink[];
  items: Map<string, SimItemInfo>;
  /** flag name → the Location whose collection sets it (non-volatile). */
  flagSetters: Map<LocationId, string[]>;
  puzzles: PuzzleInstance[];
  spheres: string[][];
}

export function buildSimWorld(rr: ReachResult): SimWorld {
  const sphereOf = new Map<LocationId, number>();
  rr.meta.spheres.forEach((s, i) => {
    for (const l of s) sphereOf.set(l, i);
  });

  const nodes = new Map<RegionId, SimNode>();
  for (const r of rr.graph.regions) nodes.set(r.id, { id: r.id, role: r.role, locations: [] });
  for (const loc of rr.graph.locations) {
    const node = nodes.get(loc.region);
    if (!node) continue;
    const sl: SimLocation = { id: loc.id };
    const itemId = rr.placement.get(loc.id);
    if (itemId !== undefined) sl.itemId = itemId;
    if (loc.bonus !== undefined) sl.bonus = loc.bonus;
    const sphere = sphereOf.get(loc.id);
    if (sphere !== undefined) sl.sphere = sphere;
    node.locations.push(sl);
  }

  const flagSetters = new Map<LocationId, string[]>();
  for (const f of rr.graph.flags) {
    if (f.volatile) continue;
    const list = flagSetters.get(f.setBy) ?? [];
    list.push(f.name);
    flagSetters.set(f.setBy, list);
  }

  const items = new Map<string, SimItemInfo>();
  for (const it of rr.items) items.set(it.id, { id: it.id, class: it.class, grants: it.grants ?? [] });

  const links: SimLink[] = rr.graph.edges.map((e) => (e.oneWay !== undefined ? { from: e.from, to: e.to, rule: e.rule, oneWay: e.oneWay } : { from: e.from, to: e.to, rule: e.rule }));

  return {
    start: rr.graph.start,
    terminal: rr.graph.regions.find((r) => r.role === "terminal")?.id,
    startHeld: heldFromData(rr.meta.startHeld),
    nodes,
    links,
    items,
    flagSetters,
    puzzles: rr.puzzleInstances,
    spheres: rr.meta.spheres,
  };
}

/** Neighbors of `id`: forward links gated by their rule, reverse links open unless one-way. */
export function neighbors(world: SimWorld, id: RegionId, held: Held): Array<{ to: RegionId; ok: boolean; link: SimLink; reverse: boolean }> {
  const out: Array<{ to: RegionId; ok: boolean; link: SimLink; reverse: boolean }> = [];
  for (const l of world.links) {
    if (l.from === id) out.push({ to: l.to, ok: evalRule(l.rule, held), link: l, reverse: false });
    else if (l.to === id && !l.oneWay) out.push({ to: l.from, ok: true, link: l, reverse: true });
  }
  return out;
}

/** All regions reachable from start given held state (fixed-point, play model). */
export function reachableNodes(world: SimWorld, held: Held): Set<RegionId> {
  const reached = new Set<RegionId>([world.start]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const id of [...reached]) {
      for (const n of neighbors(world, id, held)) {
        if (n.ok && !reached.has(n.to)) {
          reached.add(n.to);
          changed = true;
        }
      }
    }
  }
  return reached;
}
