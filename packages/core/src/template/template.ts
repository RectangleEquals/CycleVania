/**
 * The Reach template DSL — the macro-structure as DATA. It declares the critical
 * path, per-node slot policy, and gating/branch/loop rules, but NOT concrete
 * capabilities (those come from the item catalog at generation time). The default
 * CrawlStar-style template (hub + N segments + capstone + terminal + vault
 * branches + back-edges) is just one instance; a game can declare any shape.
 */

import type { ItemClass } from "../fill/fill-policy.js";
import type { RegionRole } from "./role.js";

export interface TemplateNode {
  id: string;
  role: RegionRole;
  /** How many placeable locations to provision (sampled in [min, max]). */
  slots: { min: number; max: number; class?: ItemClass };
  /** Hub-style: force ≥ progressionItems + 1 always-reachable slots (fill safety). */
  bootstrap?: boolean;
}

export type BranchEntrance = "single" | "compound" | "optional-open";

export interface BranchSpec {
  /** Which critical-path node(s) a vault hangs off. */
  anchor: "any-segment" | string;
  role: "vault";
  slots: { min: number; max: number };
  entrance: BranchEntrance;
  /** How many vaults of this spec to create (sampled in [min, max]); default derived from lock count. */
  count?: { min: number; max: number };
  /** Optionally close a loop with a back-edge to an earlier segment. */
  backEdge?: { chance: number; toEarlier: boolean; gated: number };
}

export interface ReachTemplate {
  /** Ordered node ids: entry … capstone … terminal. */
  criticalPath: string[];
  nodes: Record<string, TemplateNode>;
  branches: BranchSpec[];
  gating: {
    /** Fraction of gateable critical-path edges to lock (0..1). */
    lockFraction: number;
    /** Chance a vault entrance is a compound (A∧B) lock. */
    compoundChance: number;
    keepEntryOpen: boolean;
    keepExitOpen: boolean;
  };
  loops: { guaranteeAtLeastOne: boolean };
}
