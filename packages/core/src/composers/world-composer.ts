/**
 * WorldComposer — holds the WorldSeed and all Reaches (≥1). One game may have a
 * single massive Reach (Metroid Prime), another potentially-infinite smaller ones
 * (CrawlStar). Owns cross-Reach progression: caps collected in a Reach carry into
 * the next as starting capabilities (lookbehind).
 */

import { Rng } from "../math/rng.js";
import type { Capability } from "../logic/index.js";
import type { ProgressionItem } from "../graph/region-graph.js";
import type { ItemDef } from "../registries/item-catalog.js";
import type { Registry } from "../registries/registry.js";
import { assembleWorld } from "../descriptors/assemble.js";
import type { WorldDescriptor } from "../descriptors/descriptor.js";
import type { ComposeContext } from "./context.js";
import { composeReach, type ComposeReachOptions, type ReachResult } from "./reach-composer.js";

export interface ComposeWorldOptions {
  reachCount: number;
  /** One template for all reaches, or a per-index selector. */
  template: ComposeReachOptions["template"];
  templateFor?: (reachIndex: number) => ComposeReachOptions["template"];
  /** Depth per reach index (default: index * 8). */
  depthFor?: (reachIndex: number) => number;
  styleFor?: (reachIndex: number) => string | undefined;
  /** Carry the previous reach's item capabilities into the next (lookbehind). */
  carryCaps?: boolean;
  /** World offset for reach i (default: lateral spacing so reaches don't overlap). */
  originFor?: (reachIndex: number) => [number, number, number];
  /** Lateral spacing between reaches when `originFor` is omitted (default 700). */
  reachSpacing?: number;
  /** How many progression gadgets to place per reach (rotating through the catalog). Omit = all. */
  gadgetsPerReach?: number;
  /**
   * Organic gadget economy (default ON when `gadgetsPerReach` is omitted): a seeded,
   * power-weighted, depth-scheduled draw per reach. Overrides the registry's default range.
   */
  gadgetEconomy?: { min: number; max: number };
  /** Run the volumetric geometry pass per area. */
  geometry?: boolean;
}

/** The gadgets a given reach should place, rotating through the catalog. */
export function reachGadgets(progression: readonly ProgressionItem[], perReach: number | undefined, i: number): ProgressionItem[] | undefined {
  if (!perReach || perReach <= 0 || progression.length === 0) return undefined;
  const out: ProgressionItem[] = [];
  for (let k = 0; k < perReach && k < progression.length; k++) out.push(progression[(i * perReach + k) % progression.length] as ProgressionItem);
  return out;
}

/**
 * Relative player-power of a gadget in [0,1]. Explicit `profile.power` wins; otherwise
 * it's estimated from verticality (`bias.zWeight`) + how world-reshaping its grants are.
 */
export function itemPower(def: ItemDef | undefined): number {
  const explicit = def?.profile?.power;
  if (typeof explicit === "number") return Math.max(0, Math.min(1, explicit));
  const b = def?.profile?.bias;
  const g = def?.profile?.grants;
  let est = 0.3;
  if (b?.zWeight) est += Math.min(0.35, b.zWeight * 0.5);
  if (g?.throughMatter || g?.revealHidden || g?.timeControl) est += 0.25;
  if (typeof g?.reachUp === "number") est += Math.min(0.2, g.reachUp * 0.1);
  return Math.max(0, Math.min(1, est));
}

/**
 * The organic gadget economy: choose how many gadgets this reach places (a seeded count
 * in [min,max], min ≥ 1, trending up with depth), then draw that many WITHOUT replacement
 * with a schedule — early reaches favour low-power/lateral gadgets, later reaches allow
 * high-power/vertical ones (weighted, never guaranteed → per-seed variety).
 */
export function selectReachGadgets(reg: Registry, reachIndex: number, reachCount: number, seed: string | number, econ: { min: number; max: number }): ProgressionItem[] {
  const prog = reg.items.progression;
  if (prog.length === 0) return [];
  const rng = new Rng(`${seed}:gadget-economy:${reachIndex}`);
  const depthT = reachCount > 1 ? reachIndex / (reachCount - 1) : 0;
  const lo = Math.max(1, econ.min);
  const hi = Math.max(lo, econ.max);
  let count = lo + Math.round((hi - lo) * (0.3 + 0.5 * depthT) * rng.next() + (hi - lo) * 0.15 * depthT);
  count = Math.max(lo, Math.min(hi, count, prog.length));

  const pool = [...prog];
  const pick: ProgressionItem[] = [];
  for (let n = 0; n < count && pool.length > 0; n++) {
    const weights = pool.map((it) => {
      const pw = itemPower(reg.items.defs.get(it.id));
      const w = (1 - depthT) * (1 - pw) + depthT * pw; // schedule blend
      return Math.max(0.02, w) ** 2; // sharpen the preference
    });
    const total = weights.reduce((s, x) => s + x, 0);
    let r = rng.next() * total;
    let idx = 0;
    for (; idx < pool.length; idx++) {
      r -= weights[idx] as number;
      if (r <= 0) break;
    }
    idx = Math.min(idx, pool.length - 1);
    pick.push(pool[idx] as ProgressionItem);
    pool.splice(idx, 1);
  }
  return pick;
}

export interface WorldResult {
  reaches: ReachResult[];
  /** The assembled top-level descriptor (all reaches). */
  descriptor: WorldDescriptor;
}

/**
 * The single source of truth for "which gadgets does reach `i` place?" — used by
 * `composeWorld`, `composeWorldAsync`, and `GenerationHorizon` so all three agree
 * byte-for-byte. Explicit `gadgetsPerReach` → legacy rotation; else the economy.
 */
export function gadgetsForReach(reg: Registry, opts: ComposeWorldOptions, seed: string | number, i: number): readonly ProgressionItem[] | undefined {
  return opts.gadgetsPerReach !== undefined
    ? reachGadgets(reg.items.progression, opts.gadgetsPerReach, i)
    : selectReachGadgets(reg, i, opts.reachCount, seed, opts.gadgetEconomy ?? reg.gadgetEconomy);
}

/** Build the per-reach ComposeReachOptions shared by every world/horizon path. */
export function reachOptionsFrom(ctx: ComposeContext, opts: ComposeWorldOptions, i: number, carried: ReadonlySet<Capability>, spacing: number): ComposeReachOptions {
  const style = opts.styleFor?.(i);
  const gadgets = gadgetsForReach(ctx.registry, opts, ctx.seed, i);
  return {
    template: opts.templateFor ? opts.templateFor(i) : opts.template,
    reachIndex: i,
    origin: opts.originFor ? opts.originFor(i) : [i * spacing, 0, 0],
    ...(gadgets ? { gadgets } : {}),
    ...(opts.geometry ? { geometry: true } : {}),
    ...(opts.depthFor ? { depth: opts.depthFor(i) } : {}),
    ...(opts.carryCaps ? { startCaps: [...carried] } : {}),
    ...(style ? { styleId: style } : {}),
  };
}

export function composeWorld(ctx: ComposeContext, opts: ComposeWorldOptions): WorldResult {
  if (opts.reachCount < 1) throw new Error("composeWorld: reachCount must be ≥ 1");
  const reaches: ReachResult[] = [];
  const carried = new Set<Capability>();

  const spacing = opts.reachSpacing ?? 700;
  for (let i = 0; i < opts.reachCount; i++) {
    const result = composeReach(ctx, reachOptionsFrom(ctx, opts, i, carried, spacing));
    reaches.push(result);
    if (opts.carryCaps) for (const it of result.reach.items) carried.add(it.grants);
  }

  return { reaches, descriptor: assembleWorld(reaches.map((r) => r.descriptor)) };
}
