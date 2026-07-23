/**
 * Test-only datasets, referenced by the CLI test via `./cli.fixtures.js#BROKEN`
 * (exercising the `module#export` source path + validation error reporting).
 */

import type { CapabilityDef, RegistryInput, ReachTemplate } from "@cyclevania/core";

const cap = (id: string): CapabilityDef => ({ id, held: "granted", facets: [], powerWeight: () => 0.5 });

const TINY: ReachTemplate = {
  id: "tiny",
  criticalPath: ["hub", "terminal"],
  nodes: { hub: { role: "hub", slots: { min: 4, max: 4 } }, terminal: { role: "terminal", slots: { min: 0, max: 1 } } },
  branches: [],
  gating: { lockFraction: 0, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: false, density: 0 },
};

/** Two capabilities share the id "dup" → defineRegistry throws `registry.duplicate-id`. */
export const BROKEN: RegistryInput = {
  gadgets: { capabilities: [cap("dup"), cap("dup")], gadgets: [{ id: "g", grants: ["dup"] }] },
  templatePool: { poolAt: () => [{ template: TINY, weight: 1 }] },
};
