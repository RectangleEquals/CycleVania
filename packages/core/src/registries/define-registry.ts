/**
 * `defineRegistry` — validates + normalizes all host-supplied data into a
 * `Registry` the composers consume, and computes a content `fingerprint`. Every
 * validation failure is a typed `GenError` naming the offending entry; soft
 * issues emit `warn` diagnostics (never errors). Callbacks can't be hashed, so
 * each callback-bearing def folds its `revision ?? 0` into the fingerprint.
 *
 * Spatial/geometry registries (biomes, hulls, anchor kinds, fidelity) attach in
 * M05–M09; this validates the L1/scheduling content that exists now.
 */

import { ruleCaps, usesVolatileFlag, type CapabilityId } from "../logic/index.js";
import { fnv1a } from "../math/index.js";
import { GenError } from "../errors.js";
import { Diag, DEFAULT_DIAGNOSTICS, type DiagnosticsConfig } from "../diagnostics.js";
import type { CapabilityDef, GadgetDef, Facet, GadgetEconomyConfig } from "../capability/index.js";
import { DEFAULT_GADGET_ECONOMY } from "../capability/index.js";
import type { PuzzleDef, PuzzleEconomyConfig, SpatialRecipeDef } from "../puzzle/index.js";
import { DEFAULT_PUZZLE_ECONOMY, SHIPPED_RECIPES } from "../puzzle/index.js";
import type { ReachTemplatePool, ReachTemplate } from "../template/index.js";
import {
  DEFAULT_COMPLEXITY,
  DEFAULT_HAZARD_BASELINE,
  DEFAULT_REWARD_BASELINE,
  type ComplexityConfig,
  type BaselineConfig,
  type WorldLengthPolicy,
  type AreaCountConfig,
  type ReachModifierDef,
  type ReachModifierPolicy,
} from "../world/index.js";
import { DEFAULT_PLACEMENT_WEIGHTS, type PlacementWeightConfig } from "../fill/index.js";

export interface LockPacingConfig {
  teachTestCombine: boolean;
  combineChance: number;
}
export const DEFAULT_LOCK_PACING: LockPacingConfig = { teachTestCombine: true, combineChance: 0.3 };

export interface RegistryInput {
  gadgets: { capabilities: CapabilityDef[]; gadgets: GadgetDef[] };
  gadgetEconomy?: GadgetEconomyConfig;
  puzzles?: PuzzleDef[];
  puzzleEconomy?: PuzzleEconomyConfig;
  recipes?: SpatialRecipeDef[];
  lockPacing?: LockPacingConfig;
  templatePool: ReachTemplatePool;
  lengthPolicy?: WorldLengthPolicy;
  areaCount?: AreaCountConfig;
  complexity?: ComplexityConfig;
  hazardBaseline?: BaselineConfig;
  rewardBaseline?: BaselineConfig;
  placementWeights?: PlacementWeightConfig;
  modifierCatalog?: ReachModifierDef[];
  modifierPolicy?: ReachModifierPolicy;
  startCaps?: CapabilityId[];
  diagnostics?: DiagnosticsConfig;
}

export interface Registry {
  capabilities: CapabilityDef[];
  capById: Map<CapabilityId, CapabilityDef>;
  gadgets: GadgetDef[];
  gadgetEconomy: GadgetEconomyConfig;
  puzzles: PuzzleDef[];
  puzzleById: Map<string, PuzzleDef>;
  puzzleEconomy: PuzzleEconomyConfig;
  recipes: Map<string, SpatialRecipeDef>;
  lockPacing: LockPacingConfig;
  templatePool: ReachTemplatePool;
  lengthPolicy?: WorldLengthPolicy;
  areaCount: AreaCountConfig;
  complexity: ComplexityConfig;
  hazardBaseline: BaselineConfig;
  rewardBaseline: BaselineConfig;
  placementWeights: PlacementWeightConfig;
  modifierCatalog: ReachModifierDef[];
  modifierPolicy?: ReachModifierPolicy;
  startCaps: CapabilityId[];
  diagnostics: DiagnosticsConfig;
  fingerprint: string;
}

function assertUnique(ids: string[], what: string): void {
  const seen = new Set<string>();
  for (const id of ids) {
    if (seen.has(id)) throw new GenError("registry.duplicate-id", `duplicate ${what} id "${id}"`, { id, what });
    seen.add(id);
  }
}

function facetKey(f: Facet): unknown {
  if (f.kind === "magnitude") return { k: "m", bucket: f.bucket };
  if (f.kind === "tag") return { k: "t", tag: f.tag, cond: f.evaluate !== undefined };
  return { k: "r", pool: f.poolId, regen: f.regenHint };
}

/** Recursive key-sorted JSON (local; M08 formalizes the canonical serializer). */
function stableStringify(x: unknown): string {
  if (x === null || typeof x !== "object") return JSON.stringify(x);
  if (Array.isArray(x)) return `[${x.map(stableStringify).join(",")}]`;
  const keys = Object.keys(x as Record<string, unknown>).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stableStringify((x as Record<string, unknown>)[k])}`).join(",")}}`;
}

function detectCycle(capById: Map<CapabilityId, CapabilityDef>): void {
  const WHITE = 0;
  const GRAY = 1;
  const BLACK = 2;
  const color = new Map<CapabilityId, number>();
  const visit = (id: CapabilityId, stack: string[]): void => {
    color.set(id, GRAY);
    const def = capById.get(id);
    if (def && def.held !== "granted") {
      for (const pre of def.held.derivedFrom) {
        const c = color.get(pre) ?? WHITE;
        if (c === GRAY) throw new GenError("registry.derived-cycle", `derived capability cycle: ${[...stack, id, pre].join(" -> ")}`, { cycle: [...stack, id, pre] });
        if (c === WHITE) visit(pre, [...stack, id]);
      }
    }
    color.set(id, BLACK);
  };
  for (const id of capById.keys()) if ((color.get(id) ?? WHITE) === WHITE) visit(id, []);
}

function checkTemplate(t: ReachTemplate): void {
  if (t.criticalPath.length === 0) throw new GenError("registry.template-empty", `template "${t.id}" has an empty criticalPath`, { id: t.id });
  for (const id of t.criticalPath) {
    if (!t.nodes[id]) throw new GenError("registry.template-bad-node", `template "${t.id}" criticalPath references undefined node "${id}"`, { id: t.id, node: id });
  }
  const roles = t.criticalPath.map((id) => t.nodes[id]?.role);
  if (!roles.includes("hub")) throw new GenError("registry.template-no-hub", `template "${t.id}" has no hub node`, { id: t.id });
}

export function defineRegistry(input: RegistryInput): Registry {
  const diagCfg = input.diagnostics ?? DEFAULT_DIAGNOSTICS;
  const diag = new Diag(diagCfg);

  const capabilities = input.gadgets.capabilities;
  const gadgets = input.gadgets.gadgets;
  const puzzles = input.puzzles ?? [];
  const recipes = input.recipes ?? SHIPPED_RECIPES;

  // 1. unique ids
  assertUnique(capabilities.map((c) => c.id), "capability");
  assertUnique(gadgets.map((g) => g.id), "gadget");
  assertUnique(puzzles.map((p) => p.id), "puzzle");
  assertUnique(recipes.map((r) => r.id), "recipe");

  const capById = new Map(capabilities.map((c) => [c.id, c] as const));
  const puzzleById = new Map(puzzles.map((p) => [p.id, p] as const));
  const recipeById = new Map(recipes.map((r) => [r.id, r] as const));

  // 2. gadget grants → existing caps
  for (const g of gadgets) {
    for (const cap of g.grants) {
      if (!capById.has(cap)) throw new GenError("registry.unknown-grant", `gadget "${g.id}" grants unknown capability "${cap}"`, { gadget: g.id, cap });
    }
  }

  // 3. derivedFrom → existing caps + acyclic
  for (const c of capabilities) {
    if (c.held === "granted") continue;
    for (const pre of c.held.derivedFrom) {
      if (!capById.has(pre)) throw new GenError("registry.unknown-derived", `capability "${c.id}" derives from unknown "${pre}"`, { cap: c.id, pre });
    }
  }
  detectCycle(capById);

  // 4. puzzle validation: volatile-on-required, recipe existence, cap references
  for (const p of puzzles) {
    if (p.class === "required" && usesVolatileFlag(p.condition)) {
      throw new GenError("registry.volatile-required", `required puzzle "${p.id}" gates on a volatile flag`, { puzzle: p.id });
    }
    if (p.spatialRecipe !== undefined && !recipeById.has(p.spatialRecipe)) {
      throw new GenError("registry.unknown-recipe", `puzzle "${p.id}" names unknown recipe "${p.spatialRecipe}"`, { puzzle: p.id, recipe: p.spatialRecipe });
    }
    for (const cap of ruleCaps(p.condition)) {
      if (!capById.has(cap)) throw new GenError("registry.unknown-condition-cap", `puzzle "${p.id}" gates on unknown capability "${cap}"`, { puzzle: p.id, cap });
    }
  }

  // 5. economies / length policy sanity
  const gadgetEconomy = input.gadgetEconomy ?? DEFAULT_GADGET_ECONOMY;
  const puzzleEconomy = input.puzzleEconomy ?? DEFAULT_PUZZLE_ECONOMY;
  for (const [name, e] of [["gadgetEconomy", gadgetEconomy], ["puzzleEconomy", puzzleEconomy]] as const) {
    if (e.min < 1 || e.max < e.min) throw new GenError("registry.bad-economy", `${name} must satisfy 1 ≤ min ≤ max`, { name, e });
  }
  if (input.lengthPolicy && (input.lengthPolicy.min < 1 || (input.lengthPolicy.max !== undefined && input.lengthPolicy.max < input.lengthPolicy.min))) {
    throw new GenError("registry.bad-length", `WorldLengthPolicy must satisfy 1 ≤ min ≤ max`, { policy: input.lengthPolicy });
  }

  // 6. template static check (depth 0 pool)
  for (const entry of input.templatePool.poolAt(0)) checkTemplate(entry.template);

  // --- soft warnings (never errors) ---
  const grantedCaps = new Set(gadgets.flatMap((g) => g.grants));
  const referencedCaps = new Set(puzzles.flatMap((p) => ruleCaps(p.condition)));
  for (const c of capabilities) {
    const derivedTarget = capabilities.some((o) => o.held !== "granted" && o.held.derivedFrom.includes(c.id));
    if (!grantedCaps.has(c.id) && !referencedCaps.has(c.id) && !derivedTarget) {
      diag.warn("registry.unreachable-entry", `capability "${c.id}" is never granted or referenced`, undefined, { cap: c.id });
    }
  }
  const maxLen = input.lengthPolicy?.max;
  if (maxLen !== undefined) {
    for (const c of capabilities) if (c.guarantee && c.guarantee.withinReachLevels > maxLen) diag.warn("registry.guarantee-exceeds-length", `capability "${c.id}" guarantee exceeds World length`, undefined, { cap: c.id });
    for (const p of puzzles) if (p.guarantee && p.guarantee.withinReachLevels > maxLen) diag.warn("registry.guarantee-exceeds-length", `puzzle "${p.id}" guarantee exceeds World length`, undefined, { puzzle: p.id });
  }

  // --- fingerprint ---
  const canonical = {
    capabilities: capabilities.map((c) => ({
      id: c.id,
      held: c.held,
      facets: c.facets.map(facetKey),
      guarantee: c.guarantee ?? null,
      category: c.category ?? null,
      rev: c.revision ?? 0,
    })),
    gadgets: gadgets.map((g) => ({ id: g.id, grants: g.grants })),
    puzzles: puzzles.map((p) => ({ id: p.id, scope: p.scope, class: p.class, condition: p.condition, outcome: p.outcome, recipe: p.spatialRecipe ?? null, guarantee: p.guarantee ?? null, rev: p.revision ?? 0 })),
    recipes,
    gadgetEconomy,
    puzzleEconomy,
    lockPacing: input.lockPacing ?? DEFAULT_LOCK_PACING,
    lengthPolicy: input.lengthPolicy ?? null,
    areaCount: { min: (input.areaCount ?? { min: 5, max: 5 }).min, max: (input.areaCount ?? { min: 5, max: 5 }).max },
    complexity: input.complexity ?? DEFAULT_COMPLEXITY,
    startCaps: [...(input.startCaps ?? [])].sort(),
    modifiers: (input.modifierCatalog ?? []).map((m) => ({ id: m.id, minDepth: m.minDepth, dials: m.dials, excludesTags: m.excludesTags ?? null, tags: m.tags ?? null })),
  };
  const fingerprint = `fp_${fnv1a(stableStringify(canonical)).toString(16)}`;

  return {
    capabilities,
    capById,
    gadgets,
    gadgetEconomy,
    puzzles,
    puzzleById,
    puzzleEconomy,
    recipes: recipeById,
    lockPacing: input.lockPacing ?? DEFAULT_LOCK_PACING,
    templatePool: input.templatePool,
    ...(input.lengthPolicy ? { lengthPolicy: input.lengthPolicy } : {}),
    areaCount: input.areaCount ?? { min: 5, max: 5 },
    complexity: input.complexity ?? DEFAULT_COMPLEXITY,
    hazardBaseline: input.hazardBaseline ?? DEFAULT_HAZARD_BASELINE,
    rewardBaseline: input.rewardBaseline ?? DEFAULT_REWARD_BASELINE,
    placementWeights: input.placementWeights ?? DEFAULT_PLACEMENT_WEIGHTS,
    modifierCatalog: input.modifierCatalog ?? [],
    ...(input.modifierPolicy ? { modifierPolicy: input.modifierPolicy } : {}),
    startCaps: input.startCaps ?? [],
    diagnostics: diagCfg,
    fingerprint,
  };
}
