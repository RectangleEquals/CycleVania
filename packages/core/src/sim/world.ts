/**
 * SimWorld — a lightweight, playtest-oriented view of a composed Reach: areas,
 * their gadgets, the gated links between them, and item metadata. Built from a
 * `ReachResult` + the `Registry`. The reducer and autosolver walk it exactly the
 * way a player would (forward links gated by their rule; walking back is open
 * unless a link is one-way).
 */

import { evalRule, type Capability, type Held, type Rule } from "../logic/index.js";
import type { Registry } from "../registries/registry.js";
import type { UseEffect } from "../registries/item-catalog.js";
import type { ReachResult } from "../composers/reach-composer.js";

export interface SimGadget {
  itemId: string;
  cap?: Capability;
  locationId: string;
}

export interface SimArea {
  areaId: number;
  regionId: string;
  role: string;
  gadgets: SimGadget[];
}

export interface SimLink {
  fromAreaId: number;
  toAreaId: number;
  requires: Rule;
  oneWay?: boolean;
}

export interface SimItemInfo {
  name: string;
  class: string;
  cap?: Capability;
  use?: UseEffect;
}

export interface SimWorld {
  startAreaId: number;
  terminalAreaId: number | null;
  startCaps: Capability[];
  areas: Map<number, SimArea>;
  links: SimLink[];
  items: Map<string, SimItemInfo>;
}

export function buildSimWorld(result: ReachResult, registry: Registry): SimWorld {
  const areas = new Map<number, SimArea>();
  let terminalAreaId: number | null = null;
  for (const a of result.descriptor.areas) {
    areas.set(a.areaId, {
      areaId: a.areaId,
      regionId: a.regionId,
      role: a.role,
      gadgets: a.gadgets.map((g) => ({ itemId: g.itemId, ...(g.cap ? { cap: g.cap } : {}), locationId: g.locationId })),
    });
    if (a.role === "terminal") terminalAreaId = a.areaId;
  }

  const items = new Map<string, SimItemInfo>();
  for (const [id, def] of registry.items.defs) {
    items.set(id, {
      name: def.name ?? id,
      class: def.class,
      ...(def.grants ? { cap: def.grants } : {}),
      ...(def.use ? { use: def.use } : {}),
    });
  }

  return {
    startAreaId: result.descriptor.startAreaId,
    terminalAreaId,
    startCaps: [...registry.items.startCaps],
    areas,
    links: result.descriptor.links.map((l) => ({ fromAreaId: l.fromAreaId, toAreaId: l.toAreaId, requires: l.requires, ...(l.oneWay ? { oneWay: true } : {}) })),
    items,
  };
}

/** Areas directly reachable from `areaId`: forward links gated, reverse links open (unless one-way). */
export function neighbors(world: SimWorld, areaId: number, held: Held): Array<{ to: number; ok: boolean; link: SimLink; reverse: boolean }> {
  const out: Array<{ to: number; ok: boolean; link: SimLink; reverse: boolean }> = [];
  for (const l of world.links) {
    if (l.fromAreaId === areaId) out.push({ to: l.toAreaId, ok: evalRule(l.requires, held), link: l, reverse: false });
    else if (l.toAreaId === areaId && !l.oneWay) out.push({ to: l.fromAreaId, ok: true, link: l, reverse: true });
  }
  return out;
}

/** All areas reachable from the start given held state (fixed-point). */
export function reachableAreaIds(world: SimWorld, held: Held): Set<number> {
  const reached = new Set<number>([world.startAreaId]);
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
