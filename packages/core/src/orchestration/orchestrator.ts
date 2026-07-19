/**
 * Async orchestration façade over the sync composer cores. It only SCHEDULES and
 * STREAMS — it calls the same deterministic `composeReach`, so output is identical
 * whether run inline or offloaded. Between reaches it yields (caller-supplied, so
 * the core stays host-pure) and reports progress; it honours a cancellation token.
 */

import type { Capability } from "../logic/index.js";
import { assembleWorld } from "../descriptors/assemble.js";
import type { ComposeContext } from "../composers/context.js";
import { composeReach, type ComposeReachOptions, type ReachResult } from "../composers/reach-composer.js";
import type { ComposeWorldOptions, WorldResult } from "../composers/world-composer.js";
import type { CancellationToken } from "./cancellation.js";

export interface JobProgress {
  phase: "reach";
  index: number; // completed count
  total: number;
}

export interface OrchestrationHooks {
  signal?: CancellationToken;
  onProgress?: (p: JobProgress) => void;
  onReach?: (reach: ReachResult, index: number) => void;
  /** Cooperative yield between reaches (default: a microtask). Pass a rAF/timeout yield to breathe. */
  yield?: () => Promise<void>;
}

const microtask = (): Promise<void> => Promise.resolve();

function reachOptionsFor(opts: ComposeWorldOptions, i: number, carried: ReadonlySet<Capability>, spacing: number): ComposeReachOptions {
  const style = opts.styleFor?.(i);
  return {
    template: opts.templateFor ? opts.templateFor(i) : opts.template,
    reachIndex: i,
    origin: opts.originFor ? opts.originFor(i) : [i * spacing, 0, 0],
    ...(opts.depthFor ? { depth: opts.depthFor(i) } : {}),
    ...(opts.carryCaps ? { startCaps: [...carried] } : {}),
    ...(style ? { styleId: style } : {}),
  };
}

export async function composeWorldAsync(ctx: ComposeContext, opts: ComposeWorldOptions, hooks: OrchestrationHooks = {}): Promise<WorldResult> {
  if (opts.reachCount < 1) throw new Error("composeWorldAsync: reachCount must be ≥ 1");
  const doYield = hooks.yield ?? microtask;
  const spacing = opts.reachSpacing ?? 700;
  const reaches: ReachResult[] = [];
  const carried = new Set<Capability>();

  for (let i = 0; i < opts.reachCount; i++) {
    hooks.signal?.throwIfCancelled();
    const result = composeReach(ctx, reachOptionsFor(opts, i, carried, spacing));
    reaches.push(result);
    if (opts.carryCaps) for (const it of result.reach.items) carried.add(it.grants);
    hooks.onReach?.(result, i);
    hooks.onProgress?.({ phase: "reach", index: i + 1, total: opts.reachCount });
    await doYield();
  }

  return { reaches, descriptor: assembleWorld(reaches.map((r) => r.descriptor)) };
}

export async function composeReachAsync(ctx: ComposeContext, opts: ComposeReachOptions, hooks: OrchestrationHooks = {}): Promise<ReachResult> {
  hooks.signal?.throwIfCancelled();
  const result = composeReach(ctx, opts);
  hooks.onProgress?.({ phase: "reach", index: 1, total: 1 });
  await (hooks.yield ?? microtask)();
  return result;
}
