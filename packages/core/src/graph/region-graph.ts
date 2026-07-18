/**
 * Region graph — the abstract topology the solver reasons over. A REGION is a
 * node whose incoming edges carry access `Rule`s; a LOCATION is a placeable slot
 * (vault pedestal, cache, reward) sitting in a region. Spatial realizations
 * (areas/rooms/cells) are layered on top; the solver only sees this graph.
 */

import type { Rule, Capability } from "../logic/index.js";

export type RegionId = string;
export type LocationId = string;

export interface RegionEdge {
  from: RegionId;
  to: RegionId;
  rule: Rule;
  /** A one-way edge (drop/plunge): traversable forward only, no implied return. */
  oneWay?: boolean;
}

export interface RegionGraph {
  start: RegionId;
  regions: ReadonlySet<RegionId>;
  edges: readonly RegionEdge[];
  /** location id → the region it sits in. */
  locations: ReadonlyMap<LocationId, RegionId>;
}

/** A progression item grants one capability when collected. */
export interface ProgressionItem {
  id: string;
  grants: Capability;
}

/** location id → item id placed there. */
export type Placement = Map<LocationId, string>;
