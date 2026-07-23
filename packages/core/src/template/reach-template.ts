/**
 * The Reach template DSL — the macro-structure as DATA. It declares the critical
 * path, per-node roles + slot policy, and gating/branch/loop rules, but NEVER
 * concrete capabilities (those come from the selected content at generation time).
 * A `ReachTemplatePool` draws depth-scoped templates with seeded weights.
 */

import type { NodeRole } from "../graph/index.js";

export interface TemplateNode {
  role: NodeRole;
  /** How many placeable Locations to provision (drawn in [min, max]). */
  slots: { min: number; max: number };
}

/** A vault-style optional branch hung off a critical-path node. */
export interface BranchSpec {
  /** The criticalPath node id this branch attaches to. */
  attachTo: string;
  role: NodeRole; // usually "vault"
  entrance: "single" | "compound" | "optional-open";
  slots: { min: number; max: number };
  /** Chance the branch also closes a loop back toward the spine. */
  backEdgeChance: number;
}

export interface ReachTemplate {
  id: string;
  criticalPath: string[]; // ordered node ids: entry … capstone … terminal
  nodes: Record<string, TemplateNode>;
  branches: BranchSpec[];
  gating: {
    lockFraction: number; // fraction of gateable spine edges to lock (0..1)
    compoundChance: number; // chance a lock is a compound (A∧B)
    keepEntryOpen: boolean;
    keepExitOpen: boolean;
  };
  loops: {
    guaranteeAtLeastOne: boolean;
    density: number; // 0..1 — extra shortcut-closure attempts beyond the guarantee
  };
}

export interface ReachTemplatePool {
  poolAt(depth: number): { template: ReachTemplate; weight: number }[];
}
