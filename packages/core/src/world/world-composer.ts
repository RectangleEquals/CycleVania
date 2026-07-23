/**
 * `WorldComposer` — owns the WorldSeed and realizes Reaches strictly on request
 * (never scheduled). Each `requestReach` runs the master sequence steps 1–7
 * (validate → template → ceiling → selection → interpret → validate → fill),
 * appends exactly one request-log record, and carries state forward. L2+ stages
 * attach to the result in later milestones.
 *
 * Content SELECTION is injected (`ContentSelector`); M03 ships `verbatimSelector`
 * (fixed items + gate rules) so the world layer is fully testable before the real
 * scheduler-driven selector arrives in M04.
 */

import { CapSet, type HeldData, type Rule } from "../logic/index.js";
import { Rng } from "../math/index.js";
import { GenError } from "../errors.js";
import { validateGraph, computeSpheres, type Item, type MissionGraph, type Placement } from "../graph/index.js";
import { assumedFill, type PlacementWeightConfig, DEFAULT_PLACEMENT_WEIGHTS } from "../fill/index.js";
import { interpretTemplate, drawTemplate, type ReachTemplate, type ReachTemplatePool } from "../template/index.js";
import {
  DEFAULT_COMPLEXITY,
  actualCeiling,
  finalCeiling,
  reachLevel,
  expectedCeiling,
  type ComplexityConfig,
} from "./complexity.js";
import {
  validateModifierChoice,
  structureNudges,
  type ReachModifierDef,
  type ReachModifierPolicy,
} from "./modifiers.js";
import { requestIdentity, type ReachRequest, type ReachRequestRecord } from "./reach-request.js";
import { drawWorldLength, drawAreaCount, type AreaCountConfig, type WorldLengthPolicy } from "./length-policy.js";
import { computeVirtualSchedule, type SchedulableEntry } from "./virtual-schedule.js";
import type { ReachPortal } from "./portals.js";
import type { PuzzleInstance } from "../puzzle/puzzle-def.js";
import { aggregateBuckets } from "../capability/index.js";
import type { CapabilityDef } from "../capability/index.js";
import type { CapabilityId } from "../logic/index.js";
import { buildReachSkeleton, type AreaDialConfig, type BuildReachSkeletonOptions, type ReachSkeleton } from "../skeleton/index.js";
import { composeReachVolume } from "../volume/area-volume.js";
import { composeReachFinish, type FidelityProfile, type FinishBudgets } from "../finish/index.js";

// --- content selection (M04 replaces the default) ---

export interface SelectionContext {
  reachIndex: number;
  reachLevel: number;
  finalCeiling: number;
  isFinalReach: boolean;
  startHeld: CapSet;
  rng: Rng;
  chosenModifiers: ReachModifierDef[];
  /** id → grants across prior Reaches (0 = unplaced). */
  placedLevels: ReadonlyMap<string, number>;
  /** id → ReachLevel first eligible (default 0). */
  firstEligibleLevel: ReadonlyMap<string, number>;
  gadgetPlan: ReadonlyMap<string, number>;
  puzzlePlan: ReadonlyMap<string, number>;
  gadgetEconomyOverride?: { min?: number; max?: number };
  puzzleEconomyOverride?: { min?: number; max?: number };
}

export interface SelectionResult {
  items: Item[];
  gateRules: Rule[];
  progressionCount: number;
  /** Placed puzzle instances (carried on the ReachResult for L2/L3 realization). */
  puzzleInstances?: PuzzleInstance[];
  /** Ids the World should count toward `placedLevels` for future scheduling. */
  placedGadgetIds?: string[];
  placedPuzzleIds?: string[];
}

export interface ContentSelector {
  select(ctx: SelectionContext): SelectionResult;
}

/** A fixed selector for tests / bootstrapping: returns exactly what it was built with. */
export function verbatimSelector(items: Item[], gateRules: Rule[]): ContentSelector {
  const progressionCount = items.filter((i) => i.class === "progression").length;
  return { select: () => ({ items, gateRules, progressionCount }) };
}

// --- results & previews ---

export interface ReachMeta {
  reachIndex: number;
  requestIdentity: string;
  chosenModifiers: string[];
  finalCeiling: number;
  areaCount: number;
  spheres: string[][];
  relaxations: string[];
  startHeld: HeldData;
}

export interface ReachResult {
  meta: ReachMeta;
  graph: MissionGraph;
  placement: Placement;
  items: Item[];
  puzzleInstances: PuzzleInstance[];
  skeleton: ReachSkeleton;
  buckets: Record<string, number>;
}

/** Generation phases, yielded at boundaries by the resumable per-Reach generator. */
export type GenPhase = "template" | "selection" | "graph" | "fill" | "skeleton" | "volume" | "finish" | "assemble";

export interface ReachEnvelopePreview {
  meanNoModifiers: number;
  rangeWithMinModifiers: { low: number; high: number };
  rangeWithMaxModifiers: { low: number; high: number };
  modifierPoolSize: number;
  requiredRange: { min: number; max: number };
  plannedCapabilities?: string[];
  isDeclaredFinalReach: boolean;
}

// --- config ---

export interface WorldConfig {
  seed: string;
  templatePool: ReachTemplatePool;
  selector: ContentSelector;
  lengthPolicy?: WorldLengthPolicy;
  areaCount?: AreaCountConfig;
  complexity?: ComplexityConfig;
  placementWeights?: PlacementWeightConfig;
  modifierPolicy?: ReachModifierPolicy;
  modifierCatalog?: ReachModifierDef[];
  gadgetPool?: SchedulableEntry[];
  puzzlePool?: SchedulableEntry[];
  geometry?: boolean;
  /** Capability defs, for aggregating budget buckets that shape L2 (verticality, …). */
  capabilityDefs?: Map<CapabilityId, CapabilityDef>;
  biome?: string;
  areaDials?: AreaDialConfig;
  landmarksPerReach?: { min: number; max: number };
  /** Run the L3 volume pass (SDF fields + resolved sockets + content anchors). */
  composeVolume?: boolean;
  fidelityAngleStep?: number | null;
  /** L4 finish pass tuning (the `geometry` flag above gates it; implies composeVolume). */
  fidelity?: FidelityProfile;
  geometryBudgets?: FinishBudgets;
  /** The validated registry's content fingerprint (for output meta / reproduction). */
  registryFingerprint?: string;
  lengthPolicyRef?: WorldLengthPolicy;
}

const DEFAULT_AREA_COUNT: AreaCountConfig = { min: 5, max: 5 };

export function createWorld(config: WorldConfig): WorldComposer {
  return new WorldComposer(config);
}

export class WorldComposer {
  readonly seed: string;
  readonly drawnLength: number | undefined;
  readonly fingerprint: string;
  readonly lengthPolicy: WorldLengthPolicy | undefined;
  readonly realized = new Map<number, ReachResult>();
  readonly requestLog: ReachRequestRecord[] = [];
  readonly portals: ReachPortal[] = [];

  private readonly config: WorldConfig;
  private readonly worldRng: Rng;
  private readonly complexity: ComplexityConfig;
  private readonly modifierCatalog: Map<string, ReachModifierDef>;
  private readonly scheduleCache = new Map<string, Map<string, number>>();
  private carried = new CapSet();
  private readonly placedLevels = new Map<string, number>();

  constructor(config: WorldConfig) {
    this.config = config;
    this.seed = config.seed;
    this.worldRng = new Rng(config.seed);
    this.complexity = config.complexity ?? DEFAULT_COMPLEXITY;
    this.drawnLength = drawWorldLength(config.seed, config.lengthPolicy);
    this.fingerprint = config.registryFingerprint ?? "unfingerprinted";
    this.lengthPolicy = config.lengthPolicy;
    this.modifierCatalog = new Map((config.modifierCatalog ?? []).map((m) => [m.id, m] as const));
  }

  /** The cached, pure virtual schedule for a named pool (bounded Worlds only). */
  virtualSchedule(namespace: "gadgets" | "puzzles"): Map<string, number> {
    if (this.drawnLength === undefined) return new Map();
    const cached = this.scheduleCache.get(namespace);
    if (cached) return cached;
    const pool = namespace === "gadgets" ? this.config.gadgetPool ?? [] : this.config.puzzlePool ?? [];
    const plan = computeVirtualSchedule(this.seed, this.drawnLength, pool, `virtual-schedule:${namespace}`);
    this.scheduleCache.set(namespace, plan);
    return plan;
  }

  private validateRequest(r: ReachRequest): ReachModifierDef[] {
    if (r.reachIndex < 0) throw new GenError("request.bad-index", `negative reachIndex ${r.reachIndex}`, { r });
    if (this.realized.has(r.reachIndex)) throw new GenError("request.duplicate", `Reach ${r.reachIndex} already realized`, { reachIndex: r.reachIndex });
    if (this.drawnLength !== undefined && r.reachIndex >= this.drawnLength) {
      throw new GenError("request.past-end", `Reach ${r.reachIndex} exceeds World length ${this.drawnLength}`, { reachIndex: r.reachIndex, length: this.drawnLength });
    }
    if (r.reachIndex === 0) {
      if (r.fromReachIndex !== undefined) throw new GenError("request.bad-origin", `Reach 0 must have no fromReachIndex`, { r });
    } else {
      if (r.fromReachIndex === undefined) throw new GenError("request.missing-origin", `Reach ${r.reachIndex} needs a realized fromReachIndex`, { r });
      if (!this.realized.has(r.fromReachIndex)) throw new GenError("request.unrealized-origin", `fromReachIndex ${r.fromReachIndex} is not realized`, { r });
    }
    if (this.config.modifierPolicy) {
      return validateModifierChoice(this.config.modifierPolicy, r.reachIndex, r.chosenModifiers, this.modifierCatalog);
    }
    if (r.chosenModifiers.length > 0) {
      throw new GenError("request.no-modifier-policy", `modifiers chosen but the World has no modifier policy`, { r });
    }
    return [];
  }

  requestReach(request: ReachRequest): ReachResult {
    const g = this.reachSteps(request);
    let n = g.next();
    while (!n.done) n = g.next();
    n.value.commit();
    return n.value.result;
  }

  /**
   * The per-Reach generation as a resumable generator (yields at phase boundaries).
   * It builds entirely into locals and returns `{ result, commit }`; `commit` — the
   * only place composer state is mutated — is invoked by the driver AFTER a final
   * cancellation check, so a cancelled async run leaves the composer untouched.
   * Yields add no computation, so a drained run is byte-identical to the sync path.
   */
  *reachSteps(request: ReachRequest): Generator<GenPhase, { result: ReachResult; commit: () => void }> {
    const mods = this.validateRequest(request);
    const identity = requestIdentity(request);
    const reachRng = this.worldRng.fork(identity);
    const i = request.reachIndex;

    // 2. template
    const template: ReachTemplate = request.template ?? drawTemplate(this.config.templatePool, i, reachRng.fork("template-draw"));
    yield "template";

    // 3. finalCeiling
    const realizedPrev =
      this.realized.get(i - 1)?.meta.finalCeiling ??
      (request.fromReachIndex !== undefined ? this.realized.get(request.fromReachIndex)?.meta.finalCeiling : undefined);
    const actual = actualCeiling(i, realizedPrev, this.worldRng, this.complexity);
    const ceiling = finalCeiling(actual, mods, this.complexity);

    const isFinalReach = this.drawnLength !== undefined && i === this.drawnLength - 1;

    // 4. selection (BEFORE interpretation — gate rules reference selected content)
    const startHeld = this.carried.clone();
    const selCtx: SelectionContext = {
      reachIndex: i,
      reachLevel: reachLevel(i, this.complexity),
      finalCeiling: ceiling,
      isFinalReach,
      startHeld,
      rng: reachRng.fork("select"),
      chosenModifiers: mods,
      placedLevels: this.placedLevels,
      firstEligibleLevel: new Map(),
      gadgetPlan: this.virtualSchedule("gadgets"),
      puzzlePlan: this.virtualSchedule("puzzles"),
    };
    if (request.gadgetEconomyOverride) selCtx.gadgetEconomyOverride = request.gadgetEconomyOverride;
    if (request.puzzleEconomyOverride) selCtx.puzzleEconomyOverride = request.puzzleEconomyOverride;
    const selection = this.config.selector.select(selCtx);
    // deferred (applied in commit) so a cancelled run doesn't perturb scheduling state
    const placedIds = [...(selection.placedGadgetIds ?? []), ...(selection.placedPuzzleIds ?? [])];
    yield "selection";

    // 5. interpret
    const graph = interpretTemplate(
      template,
      { gateRules: selection.gateRules, progressionCount: selection.progressionCount },
      structureNudges(mods),
      reachRng.fork("template"),
    );
    yield "graph";

    // 6. validate — every region reachable when fully equipped this Reach
    const fully = startHeld.clone();
    for (const it of selection.items) for (const cap of it.grants ?? []) fully.add(cap, 1);
    for (const f of graph.flags) if (!f.volatile) fully.addFlag(f.name);
    validateGraph(graph, fully);

    // 7. fill
    const { placement, relaxations } = assumedFill(
      graph,
      startHeld.clone(),
      selection.items,
      reachRng.fork("fill"),
      this.config.placementWeights ?? DEFAULT_PLACEMENT_WEIGHTS,
    );

    // spheres (for meta + carried-state update)
    const spheres = computeSpheres(graph, startHeld.clone(), placement, selection.items);
    const areaCount = drawAreaCount(
      this.config.areaCount ?? DEFAULT_AREA_COUNT,
      { reachIndex: i, chosenModifiers: request.chosenModifiers, finalCeiling: ceiling },
      reachRng.fork("areacount"),
    );
    yield "fill";

    // L2 spatial skeleton (always built — cheap, no geometry)
    const buckets = this.config.capabilityDefs ? aggregateBuckets(this.config.capabilityDefs, spheres.finalHeld) : {};
    const skelOpts: BuildReachSkeletonOptions = { finalCeiling: ceiling, buckets, puzzleInstances: selection.puzzleInstances ?? [] };
    if (this.config.biome !== undefined) skelOpts.biome = this.config.biome;
    if (this.config.areaDials !== undefined) skelOpts.dialCfg = this.config.areaDials;
    if (this.config.landmarksPerReach !== undefined) skelOpts.landmarksPerReach = this.config.landmarksPerReach;
    const skeleton = buildReachSkeleton(graph, `${identity}:skeleton`, skelOpts);
    yield "skeleton";

    // L3 volume pass (gated — SDF fields + resolved sockets + anchors; geometry implies it)
    if (this.config.composeVolume || this.config.geometry) {
      composeReachVolume(skeleton, graph, placement, {
        seed: `${identity}:volume`,
        fidelityAngleStep: this.config.fidelityAngleStep ?? 5,
        spheres: spheres.spheres,
      });
    }
    yield "volume";
    // L4 finish pass (gated by geometry — mesh → kit + occupancy + dressing)
    if (this.config.geometry) {
      composeReachFinish(skeleton, {
        seed: `${identity}:finish`,
        ...(this.config.fidelity ? { fidelity: this.config.fidelity } : {}),
        ...(this.config.geometryBudgets ? { budgets: this.config.geometryBudgets } : {}),
      });
    }
    yield "finish";

    const meta: ReachMeta = {
      reachIndex: i,
      requestIdentity: identity,
      chosenModifiers: [...request.chosenModifiers].sort(),
      finalCeiling: ceiling,
      areaCount,
      spheres: spheres.spheres,
      relaxations: [...relaxations, ...skeleton.relaxations, ...skeleton.areas.flatMap((a) => a.relaxations)],
      startHeld: startHeld.toData(),
    };
    const result: ReachResult = {
      meta,
      graph,
      placement,
      items: selection.items,
      puzzleInstances: selection.puzzleInstances ?? [],
      skeleton,
      buckets,
    };

    yield "assemble";
    const commit = (): void => {
      for (const id of placedIds) this.placedLevels.set(id, (this.placedLevels.get(id) ?? 0) + 1);
      this.realized.set(i, result);
      this.requestLog.push({ request, identity });
      this.carried = spheres.finalHeld;
      if (graph.regions.some((r) => r.role === "terminal")) {
        this.portals.push({ fromReach: i, toReach: i + 1, oneWay: false, fromSpaceHint: "terminal", toSpaceHint: "hub" });
      }
    };
    return { result, commit };
  }

  /** Add a host-authored portal between two already-realized Reaches. */
  addPortal(p: ReachPortal): void {
    if (!this.realized.has(p.fromReach) || !this.realized.has(p.toReach)) {
      throw new GenError("portal.unrealized", `portal endpoints must both be realized`, { p });
    }
    this.portals.push(p);
  }

  /** Pure, read-only lookahead — draws NO entropy RNG (never perturbs generation). */
  previewReachEnvelope(index: number): ReachEnvelopePreview {
    const cfg = this.complexity;
    const mean = expectedCeiling(index, cfg);
    const low = mean * (1 - cfg.JITTER_FRAC);
    const high = mean * (1 + cfg.JITTER_FRAC);

    const pool = this.config.modifierPolicy?.poolAt(index) ?? [];
    const range = this.config.modifierPolicy?.requiredRange(index) ?? { min: 0, max: 0 };
    const impact = (m: ReachModifierDef): number => (m.dials.complexity?.additive ?? 0) + (m.dials.complexity?.multiplier ?? 0) * mean;
    const byImpact = [...pool].sort((a, b) => impact(a) - impact(b));
    const minMods = byImpact.slice(0, Math.min(range.min, byImpact.length));
    const maxMods = byImpact.slice(Math.max(0, byImpact.length - Math.min(range.max, byImpact.length)));

    const preview: ReachEnvelopePreview = {
      meanNoModifiers: mean,
      rangeWithMinModifiers: { low: finalCeiling(low, minMods, cfg), high: finalCeiling(high, minMods, cfg) },
      rangeWithMaxModifiers: { low: finalCeiling(low, maxMods, cfg), high: finalCeiling(high, maxMods, cfg) },
      modifierPoolSize: pool.length,
      requiredRange: range,
      isDeclaredFinalReach: this.drawnLength !== undefined && index === this.drawnLength - 1,
    };
    if (this.drawnLength !== undefined && (this.config.gadgetPool?.length ?? 0) > 0) {
      const plan = this.virtualSchedule("gadgets");
      const planned = [...plan.entries()].filter(([, slot]) => slot === index).map(([id]) => id);
      preview.plannedCapabilities = planned;
    }
    return preview;
  }
}
