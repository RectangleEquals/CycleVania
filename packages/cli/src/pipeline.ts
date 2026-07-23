/**
 * Shared generation pipeline used by generate/report/soak/diff — build a validated
 * registry from a resolved source, then realize a world either by replaying a
 * bundle's request log or by generating N chained Reaches from scratch.
 */

import { defineRegistry, worldFromRegistry } from "@cyclevania/core";
import type { DiagnosticsConfig, Registry, WorldComposer, ReachResult, WorldFromRegistryOptions } from "@cyclevania/core";
import type { ResolvedSource } from "./sources.js";

export function makeRegistry(src: ResolvedSource, diagnostics?: DiagnosticsConfig): Registry {
  if (!src.input) throw new Error("this source has no dataset (it is a world descriptor) — nothing to generate");
  return defineRegistry(diagnostics ? { ...src.input, diagnostics } : src.input);
}

export interface RealizeOptions {
  seed: string;
  reaches?: number;
  geometry?: boolean;
}

export interface RealizeResult {
  world: WorldComposer;
  reaches: ReachResult[];
  usedWorldOptions: WorldFromRegistryOptions;
}

export function realize(registry: Registry, src: ResolvedSource, opts: RealizeOptions): RealizeResult {
  const usedWorldOptions: WorldFromRegistryOptions = { ...(src.world ?? {}), ...(opts.geometry !== undefined ? { geometry: opts.geometry } : {}) };
  const world = worldFromRegistry(registry, opts.seed, usedWorldOptions);

  const reaches: ReachResult[] = [];
  const replay = src.requestLog && src.requestLog.length > 0 && opts.reaches === undefined;
  if (replay) {
    for (const rec of src.requestLog!) reaches.push(world.requestReach(rec.request));
  } else {
    const n = Math.max(1, opts.reaches ?? 1);
    for (let i = 0; i < n; i++) {
      if (world.drawnLength !== undefined && i >= world.drawnLength) break;
      const req = i === 0 ? { reachIndex: 0, chosenModifiers: [] } : { reachIndex: i, fromReachIndex: i - 1, chosenModifiers: [] };
      reaches.push(world.requestReach(req));
    }
  }
  return { world, reaches, usedWorldOptions };
}
