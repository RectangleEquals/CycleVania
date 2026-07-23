/**
 * The mission graph (L1) — the abstract topology the solver reasons over, before
 * any coordinate exists. A REGION is a node (mapping 1:1 onto an Area in L2); an
 * EDGE is a directed, rule-gated connection; a FLAG is a named world fact with
 * provenance; a LOCATION is a placeable slot (which may carry its own gate and/or
 * be a bonus/optional slot).
 *
 * NOTE on shape: the redesign lists `locations` loosely as `Map<LocationId,
 * RegionId>`, but also requires per-Location gates (future-cap "teases") and a
 * bonus flag (optional content excluded from the solvability clause). This module
 * uses the richer `LocationDef[]` that supports every feature the redesign
 * describes; `locationRegion` gives the simple id→region view when that's all a
 * caller needs.
 */

import type { Rule, CapabilityId } from "../logic/index.js";

export type RegionId = string;
export type LocationId = string;

/** Region roles a template assigns (L2 adds "junction" to Spaces, not Regions). */
export type NodeRole = "hub" | "segment" | "gate" | "vault" | "capstone" | "terminal";

export type ItemClass = "progression" | "useful" | "filler" | "bonus";

export interface Region {
  id: RegionId;
  role: NodeRole;
}

export interface Edge {
  from: RegionId;
  to: RegionId;
  rule: Rule;
  /** A one-way edge (drop/plunge): forward only, no implied return. */
  oneWay?: boolean;
}

/** A named world fact, with the Location (or, later, Puzzle) that sets it. */
export interface FlagDef {
  name: string;
  setBy: LocationId;
  /** Timed/one-shot: excluded from the baseline solvability proof. */
  volatile?: boolean;
}

export interface LocationDef {
  id: LocationId;
  region: RegionId;
  /** Optional per-Location gate (e.g. a future-cap tease); absent = always open. */
  gate?: Rule;
  /** Optional/side content: excluded from the "every Location reachable" clause. */
  bonus?: boolean;
}

export interface MissionGraph {
  regions: Region[];
  edges: Edge[];
  flags: FlagDef[];
  locations: LocationDef[];
  start: RegionId;
}

/** Something placed at a Location; a progression item grants capabilities. */
export interface Item {
  id: string;
  class: ItemClass;
  grants?: CapabilityId[];
}

/** location id → item id placed there. */
export type Placement = Map<LocationId, string>;

/** Simple id→region view. */
export function locationRegion(g: MissionGraph, id: LocationId): RegionId | undefined {
  return g.locations.find((l) => l.id === id)?.region;
}
