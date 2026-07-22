/**
 * GenerationHorizon — a lazy, cached cursor over a World's Reaches. A game builds
 * only the Reaches near the player and pulls the next one on demand (each Reach is
 * generated deterministically from the run seed), so the world streams in instead
 * of composing all at once. Mirrors "generate the next Reach from within the most
 * recent Sanctum."
 */

import type { Capability } from "../logic/index.js";
import type { ComposeContext } from "../composers/context.js";
import { composeReach, type ReachResult } from "../composers/reach-composer.js";
import { reachOptionsFrom, type ComposeWorldOptions } from "../composers/world-composer.js";

export class GenerationHorizon {
  private cache = new Map<number, ReachResult>();
  private carried = new Set<Capability>();
  private highest = -1;
  private readonly spacing: number;

  constructor(
    private ctx: ComposeContext,
    private opts: ComposeWorldOptions,
  ) {
    this.spacing = opts.reachSpacing ?? 700;
  }

  has(i: number): boolean {
    return this.cache.has(i);
  }

  /** Compose (or return cached) Reach `i`; composes predecessors first if caps carry. */
  reach(i: number): ReachResult {
    const cached = this.cache.get(i);
    if (cached) return cached;
    if (this.opts.carryCaps) for (let k = this.highest + 1; k < i; k++) this.reach(k);

    const result = composeReach(this.ctx, reachOptionsFrom(this.ctx, this.opts, i, this.carried, this.spacing));

    this.cache.set(i, result);
    if (this.opts.carryCaps) for (const it of result.reach.items) this.carried.add(it.grants);
    this.highest = Math.max(this.highest, i);
    return result;
  }

  /** Ensure Reaches `from … from+radius` are composed; returns them. */
  prefetch(from: number, radius: number): ReachResult[] {
    const out: ReachResult[] = [];
    for (let i = Math.max(0, from); i <= from + radius; i++) out.push(this.reach(i));
    return out;
  }

  /** Drop cached Reaches outside `[keepFrom, keepTo]` to bound memory. */
  evictOutside(keepFrom: number, keepTo: number): void {
    for (const i of [...this.cache.keys()]) if (i < keepFrom || i > keepTo) this.cache.delete(i);
  }
}
