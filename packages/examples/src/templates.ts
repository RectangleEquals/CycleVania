/**
 * Example Reach templates + a depth-scoped pool. Three macro-shapes: a lean linear
 * spine, a hub-and-spoke with vaults, and a loop-heavy dungeon.
 */

import type { ReachTemplate, ReachTemplatePool, NodeRole } from "@cyclevania/core";

const node = (role: NodeRole, min: number, max: number) => ({ role, slots: { min, max } });

export const LINEAR: ReachTemplate = {
  id: "linear",
  criticalPath: ["hub", "s1", "gate1", "s2", "capstone", "terminal"],
  nodes: {
    hub: node("hub", 6, 8),
    s1: node("segment", 1, 3),
    gate1: node("gate", 1, 2),
    s2: node("segment", 1, 3),
    capstone: node("capstone", 1, 2),
    terminal: node("terminal", 0, 1),
  },
  branches: [{ attachTo: "s1", role: "vault", entrance: "single", slots: { min: 1, max: 2 }, backEdgeChance: 0.3 }],
  gating: { lockFraction: 0.5, compoundChance: 0.15, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true, density: 0.25 },
};

export const HUB_SPOKE: ReachTemplate = {
  id: "hub-spoke",
  criticalPath: ["hub", "gate1", "s1", "capstone", "terminal"],
  nodes: {
    hub: node("hub", 8, 10),
    gate1: node("gate", 1, 1),
    s1: node("segment", 2, 3),
    capstone: node("capstone", 1, 2),
    terminal: node("terminal", 0, 1),
  },
  branches: [
    { attachTo: "hub", role: "vault", entrance: "single", slots: { min: 1, max: 3 }, backEdgeChance: 0.4 },
    { attachTo: "hub", role: "vault", entrance: "compound", slots: { min: 1, max: 2 }, backEdgeChance: 0.2 },
    { attachTo: "s1", role: "vault", entrance: "optional-open", slots: { min: 1, max: 2 }, backEdgeChance: 0.5 },
  ],
  gating: { lockFraction: 0.4, compoundChance: 0.2, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true, density: 0.35 },
};

export const LOOP_HEAVY: ReachTemplate = {
  id: "loop-heavy",
  criticalPath: ["hub", "s1", "s2", "gate1", "s3", "capstone", "terminal"],
  nodes: {
    hub: node("hub", 7, 9),
    s1: node("segment", 1, 2),
    s2: node("segment", 1, 2),
    gate1: node("gate", 1, 1),
    s3: node("segment", 1, 2),
    capstone: node("capstone", 1, 2),
    terminal: node("terminal", 0, 1),
  },
  branches: [{ attachTo: "s2", role: "vault", entrance: "single", slots: { min: 1, max: 2 }, backEdgeChance: 0.6 }],
  gating: { lockFraction: 0.6, compoundChance: 0.25, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true, density: 0.8 },
};

export const EXAMPLE_TEMPLATE_POOL: ReachTemplatePool = {
  poolAt: (depth) =>
    depth < 2
      ? [{ template: LINEAR, weight: 3 }, { template: HUB_SPOKE, weight: 1 }]
      : [{ template: LINEAR, weight: 1 }, { template: HUB_SPOKE, weight: 2 }, { template: LOOP_HEAVY, weight: 2 }],
};
