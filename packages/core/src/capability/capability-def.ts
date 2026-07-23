/**
 * Capabilities carry FACETS, and Facets are evaluated, not stored. A capability's
 * generation consequence is one of three answer-shapes: how much (Magnitude → a
 * budget bucket), can-this-happen-here (Tag), or what-pool (Resource). CycleVania
 * never knows what a capability *does* in gameplay — only its id, its Facets, and
 * its scheduling power.
 */

import type { CapabilityId } from "../logic/index.js";

/** The closed set of built-in budget buckets L2 consumes directly. */
export const BUILTIN_BUCKETS = [
  "traversal.zUp",
  "traversal.zDown",
  "traversal.xyGap",
  "traversal.permeate",
  "traversal.perceive",
  "traversal.timeHazard",
  "traversal.weight",
  "challenge.offense",
  "challenge.defense",
] as const;

export type BuiltinBucket = (typeof BUILTIN_BUCKETS)[number];
export type Bucket = BuiltinBucket | `custom.${string}`;

export interface FacetContext {
  level: number;
  resource?: { charge: number; capacity: number };
  held: ReadonlySet<CapabilityId>;
}

export interface MagnitudeFacet {
  kind: "magnitude";
  bucket: Bucket;
  /** Returns a magnitude in CYCLEVANIA's world units (host converts at the boundary). */
  evaluate(ctx: FacetContext): number;
}

export interface TagFacet {
  kind: "tag";
  tag: string;
  /** Omit for "always active once held". */
  evaluate?(ctx: FacetContext): boolean;
}

export interface ResourceFacet {
  kind: "resource";
  poolId: string;
  capacity(ctx: FacetContext): number;
  regenHint: "site" | "time" | "kill" | "none";
}

export type Facet = MagnitudeFacet | TagFacet | ResourceFacet;

export interface CapabilityDef {
  id: CapabilityId;
  /** Granted by a pickup, or DERIVED from holding others (combos). */
  held: "granted" | { derivedFrom: CapabilityId[]; minLevels?: Partial<Record<CapabilityId, number>> };
  facets: Facet[];
  /** 0..1 scheduling power at a given level. Required. */
  powerWeight: (level: number) => number;
  /** Optional pity window: force placement if still unplaced this many ReachLevels after eligible. */
  guarantee?: { withinReachLevels: number };
  /** Host-only authoring label — stored, never interpreted. */
  category?: string;
  /** Host bumps when a callback's behavior changes (folds into the fingerprint). */
  revision?: number;
}

/** One pickup → one or more capabilities. */
export interface GadgetDef {
  id: string;
  grants: CapabilityId[];
}
