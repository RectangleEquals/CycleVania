/**
 * WorldComposer — holds the WorldSeed and all Reaches (≥1). One game may have a
 * single massive Reach (Metroid Prime), another potentially-infinite smaller ones
 * (CrawlStar). Owns cross-Reach progression: caps collected in a Reach carry into
 * the next as starting capabilities (lookbehind).
 */

import type { Capability } from "../logic/index.js";
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
}

export interface WorldResult {
  reaches: ReachResult[];
  /** The assembled top-level descriptor (all reaches). */
  descriptor: WorldDescriptor;
}

export function composeWorld(ctx: ComposeContext, opts: ComposeWorldOptions): WorldResult {
  if (opts.reachCount < 1) throw new Error("composeWorld: reachCount must be ≥ 1");
  const reaches: ReachResult[] = [];
  const carried = new Set<Capability>();

  const spacing = opts.reachSpacing ?? 700;
  for (let i = 0; i < opts.reachCount; i++) {
    const template = opts.templateFor ? opts.templateFor(i) : opts.template;
    const styleId = opts.styleFor?.(i);
    const origin = opts.originFor ? opts.originFor(i) : ([i * spacing, 0, 0] as [number, number, number]);
    const result = composeReach(ctx, {
      template,
      reachIndex: i,
      origin,
      ...(opts.depthFor ? { depth: opts.depthFor(i) } : {}),
      ...(opts.carryCaps ? { startCaps: [...carried] } : {}),
      ...(styleId ? { styleId } : {}),
    });
    reaches.push(result);
    if (opts.carryCaps) for (const it of result.reach.items) carried.add(it.grants);
  }

  return { reaches, descriptor: assembleWorld(reaches.map((r) => r.descriptor)) };
}
