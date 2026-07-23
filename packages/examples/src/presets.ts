/**
 * The three shipped presets — one point each on the fidelity spectrum, proving the
 * same pipeline (fed different data) makes a boxy crawler, a balanced faceted
 * metroidvania, or an epic-scale organic world. They share the example catalogs
 * where possible so diffs isolate the SPATIAL dials.
 */

import {
  DEFAULT_AREA_DIALS,
  DEFAULT_COMPLEXITY,
  DEFAULT_FIDELITY,
  defineRegistry,
  worldFromRegistry,
  type FidelityProfile,
  type GadgetDef,
  type RegistryInput,
  type WorldComposer,
  type WorldFromRegistryOptions,
} from "@cyclevania/core";
import { CAPABILITIES, GADGET_CATALOG } from "./gadget-catalog.js";
import { PUZZLES } from "./puzzle-catalog.js";
import { EXAMPLE_TEMPLATE_POOL, LINEAR } from "./templates.js";
import { MODIFIERS, MODIFIER_POLICY } from "./modifiers.js";

export interface Preset {
  name: string;
  input: RegistryInput;
  world: WorldFromRegistryOptions;
}

const FIDELITY_90: FidelityProfile = { angleStepDeg: 90, voxelRes: 1.5, maxDim: 48, snapNormals: true };

// crawler: lateral gadgets only (no vertical z-budget), boxy + orthogonal
const LATERAL_CAPS = CAPABILITIES.filter((c) => !c.facets.some((f) => f.kind === "magnitude" && (f.bucket === "traversal.zUp" || f.bucket === "traversal.zDown")));
const LATERAL_GADGETS: GadgetDef[] = LATERAL_CAPS.map((c) => ({ id: `the-${c.id}`, grants: [c.id] }));

export const CRAWLER: Preset = {
  name: "crawler",
  input: {
    gadgets: { capabilities: LATERAL_CAPS, gadgets: LATERAL_GADGETS },
    gadgetEconomy: { min: 1, max: 2 },
    complexity: { ...DEFAULT_COMPLEXITY, BaseCeiling: 55, HARD_MAX: 200, ABSOLUTE_HARD_MAX: 260 },
    templatePool: { poolAt: () => [{ template: LINEAR, weight: 1 }] },
    areaCount: { min: 3, max: 4 },
    lengthPolicy: { min: 3, max: 5 },
  },
  world: {
    fidelity: FIDELITY_90,
    fidelityAngleStep: 90,
    landmarksPerReach: { min: 0, max: 0 },
    areaDials: { ...DEFAULT_AREA_DIALS, baseOutdoorChance: 0, baseLargeChance: 0.05, K_Z: 0.3 },
  },
};

export const CLASSIC: Preset = {
  name: "classic",
  input: {
    gadgets: GADGET_CATALOG,
    gadgetEconomy: { min: 1, max: 3 },
    puzzles: PUZZLES,
    templatePool: EXAMPLE_TEMPLATE_POOL,
    modifierCatalog: MODIFIERS,
    modifierPolicy: MODIFIER_POLICY,
    areaCount: { min: 5, max: 5 },
    lengthPolicy: { min: 4, max: 6 },
  },
  world: { fidelity: DEFAULT_FIDELITY, fidelityAngleStep: 5, landmarksPerReach: { min: 1, max: 1 } },
};

export const PRIME: Preset = {
  name: "prime",
  input: {
    gadgets: GADGET_CATALOG,
    gadgetEconomy: { min: 1, max: 3 },
    puzzles: PUZZLES,
    templatePool: EXAMPLE_TEMPLATE_POOL,
    modifierCatalog: MODIFIERS,
    modifierPolicy: MODIFIER_POLICY,
    complexity: { ...DEFAULT_COMPLEXITY, BaseCeiling: 140, K_MUL: 0.5 },
    areaCount: { min: 3, max: 5 },
    lengthPolicy: { min: 5, max: 7 },
  },
  world: {
    fidelity: DEFAULT_FIDELITY,
    fidelityAngleStep: 5,
    landmarksPerReach: { min: 1, max: 2 },
    areaDials: { ...DEFAULT_AREA_DIALS, baseOutdoorChance: 0.4, baseLargeChance: 0.3, K_Z: 2 },
  },
};

export const PRESETS: Record<string, Preset> = { crawler: CRAWLER, classic: CLASSIC, prime: PRIME };

/** Build a WorldComposer from a preset (fidelity/dials applied), with an optional geometry override. */
export function presetWorld(preset: Preset, seed: string, override?: { geometry?: boolean; volume?: boolean }): WorldComposer {
  const registry = defineRegistry(preset.input);
  return worldFromRegistry(registry, seed, { ...preset.world, ...override });
}
