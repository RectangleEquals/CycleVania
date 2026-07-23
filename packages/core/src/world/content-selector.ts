/**
 * The registry-driven content selector — replaces `verbatimSelector`. Runs both
 * schedulers (gadgets + puzzles) against the registry, builds progression items
 * from chosen gadgets, and assembles gate rules in teach → test → combine order
 * over the reach's newly-introduced FACET-FUL capabilities (facet-less "key" caps
 * like collectathon fragments never get a teach gate — their count-lock puzzle is
 * the gate), then appends required puzzle conditions. Puzzle instances share their
 * `condition` Rule BY REFERENCE with the bound edge.
 */

import { have, and, type CapabilityId, type Rule } from "../logic/index.js";
import type { Item } from "../graph/index.js";
import { gadgetEntry, type GadgetEconomyConfig } from "../capability/index.js";
import { puzzleEntry, instantiate, type PuzzleInstance } from "../puzzle/index.js";
import { scheduleDraw, type ScheduleContext } from "./scheduling.js";
import { resolveEconomy } from "./modifiers.js";
import { createWorld, type ContentSelector, type SelectionContext, type WorldComposer, type WorldConfig } from "./world-composer.js";
import type { Registry } from "../registries/define-registry.js";
import type { FidelityProfile, FinishBudgets } from "../finish/index.js";
import type { AreaDialConfig } from "../skeleton/index.js";

export interface WorldFromRegistryOptions {
  geometry?: boolean;
  volume?: boolean;
  fidelity?: FidelityProfile;
  fidelityAngleStep?: number | null;
  geometryBudgets?: FinishBudgets;
  areaDials?: AreaDialConfig;
  landmarksPerReach?: { min: number; max: number };
  biome?: string;
}

function schedCtx(ctx: SelectionContext, plan: ReadonlyMap<string, number>): ScheduleContext {
  return {
    reachLevel: ctx.reachLevel,
    reachIndex: ctx.reachIndex,
    isFinalReach: ctx.isFinalReach,
    placedLevels: ctx.placedLevels,
    firstEligibleLevel: ctx.firstEligibleLevel,
    virtualPlan: plan,
  };
}

export function createRegistrySelector(registry: Registry): ContentSelector {
  const gadgetById = new Map(registry.gadgets.map((g) => [g.id, g] as const));
  const requiredPuzzleIds = new Set(registry.puzzles.filter((p) => p.class === "required").map((p) => p.id));

  return {
    select(ctx) {
      // --- gadgets ---
      const gEcon: GadgetEconomyConfig = resolveEconomy(registry.gadgetEconomy, ctx.chosenModifiers, "gadgetEconomy", ctx.gadgetEconomyOverride);
      const gadgetPool = registry.gadgets
        .filter((g) => (ctx.placedLevels.get(g.id) ?? 0) === 0)
        .map((g) => gadgetEntry(g, registry.capById));
      const gDraw = scheduleDraw(gadgetPool, gEcon, schedCtx(ctx, ctx.gadgetPlan), ctx.rng.fork("gadget-schedule"));
      const gadgetIds = [...gDraw.chosen, ...gDraw.pityForced, ...gDraw.sweepForced];

      const items: Item[] = [];
      for (const id of gadgetIds) {
        const g = gadgetById.get(id);
        if (g) items.push({ id, class: "progression", grants: g.grants });
      }

      // --- puzzles ---
      const pEcon = resolveEconomy(registry.puzzleEconomy, ctx.chosenModifiers, "puzzleEconomy", ctx.puzzleEconomyOverride);
      const puzzlePool = registry.puzzles
        .filter((p) => (ctx.placedLevels.get(p.id) ?? 0) === 0)
        .map((p) => puzzleEntry(p));
      const pDraw = scheduleDraw(puzzlePool, pEcon, schedCtx(ctx, ctx.puzzlePlan), ctx.rng.fork("puzzle-schedule"));
      const puzzleIds = [...pDraw.chosen, ...pDraw.pityForced, ...pDraw.sweepForced.filter((id) => requiredPuzzleIds.has(id))];
      const instances: PuzzleInstance[] = [];
      for (const id of puzzleIds) {
        const def = registry.puzzleById.get(id);
        if (def) instances.push(instantiate(def, `${id}#r${ctx.reachIndex}`));
      }

      // --- gate rules: teach → test → combine over facet-ful new caps ---
      const newCaps: CapabilityId[] = [];
      for (const id of gadgetIds) {
        const g = gadgetById.get(id);
        if (!g) continue;
        for (const cap of g.grants) {
          const def = registry.capById.get(cap);
          if (def && def.facets.length > 0 && !ctx.startHeld.hasCap(cap) && !newCaps.includes(cap)) newCaps.push(cap);
        }
      }
      const gateRules: Rule[] = [];
      const carriedCaps = ctx.startHeld.capIds();
      const combineRng = ctx.rng.fork("combine");
      for (let k = 0; k < newCaps.length; k++) {
        const capK = newCaps[k];
        if (capK === undefined) continue;
        let r: Rule = have(capK);
        if (registry.lockPacing.teachTestCombine && k > 0 && combineRng.chance(registry.lockPacing.combineChance)) {
          const earlier = [...carriedCaps, ...newCaps.slice(0, k)];
          if (earlier.length > 0) r = and(have(capK), have(combineRng.pick(earlier)));
        }
        gateRules.push(r);
      }
      // required puzzle conditions become locks (shared Rule reference)
      for (const inst of instances) if (inst.class === "required") gateRules.push(inst.condition);

      return {
        items,
        gateRules,
        progressionCount: items.length,
        puzzleInstances: instances,
        placedGadgetIds: gadgetIds,
        placedPuzzleIds: puzzleIds,
      };
    },
  };
}

/** Build a WorldComposer directly from a validated Registry (the common host path). */
export function worldFromRegistry(registry: Registry, seed: string, opts?: WorldFromRegistryOptions): WorldComposer {
  const gadgetPool = registry.gadgets.map((g) => gadgetEntry(g, registry.capById));
  const puzzlePool = registry.puzzles.map((p) => puzzleEntry(p));
  const config: WorldConfig = {
    seed,
    templatePool: registry.templatePool,
    selector: createRegistrySelector(registry),
    areaCount: registry.areaCount,
    complexity: registry.complexity,
    placementWeights: registry.placementWeights,
    modifierCatalog: registry.modifierCatalog,
    capabilityDefs: registry.capById,
    registryFingerprint: registry.fingerprint,
    gadgetPool,
    puzzlePool,
    ...(registry.lengthPolicy ? { lengthPolicy: registry.lengthPolicy } : {}),
    ...(registry.modifierPolicy ? { modifierPolicy: registry.modifierPolicy } : {}),
    ...(opts?.geometry !== undefined ? { geometry: opts.geometry } : {}),
    ...(opts?.volume !== undefined ? { composeVolume: opts.volume } : {}),
    ...(opts?.fidelity !== undefined ? { fidelity: opts.fidelity } : {}),
    ...(opts?.fidelityAngleStep !== undefined ? { fidelityAngleStep: opts.fidelityAngleStep } : {}),
    ...(opts?.geometryBudgets !== undefined ? { geometryBudgets: opts.geometryBudgets } : {}),
    ...(opts?.areaDials !== undefined ? { areaDials: opts.areaDials } : {}),
    ...(opts?.landmarksPerReach !== undefined ? { landmarksPerReach: opts.landmarksPerReach } : {}),
    ...(opts?.biome !== undefined ? { biome: opts.biome } : {}),
  };
  return createWorld(config);
}
