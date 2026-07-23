/**
 * GenerationHorizon — the host's agent issuing real ReachRequests EARLY. Reaches
 * are still requested (never scheduled): the horizon calls `requestReachAsync`
 * with host-authored requests, caches the results, and evicts by policy (evicted
 * descriptors stay regenerable from identity). Because prefetched requests enter
 * the request log like any other, determinism is untouched.
 */

import type { ReachRequest, ReachResult, WorldComposer } from "../world/index.js";
import { requestReachAsync, type OrchestrationHooks } from "./async.js";

export interface HorizonPolicy {
  ahead: number;
  /** HOST-authored: the horizon can't invent modifier choices. */
  requestFor: (nextIndex: number) => ReachRequest;
  evictBehind?: number;
}

export class GenerationHorizon {
  private readonly cache = new Map<number, ReachResult>();

  constructor(
    private readonly world: WorldComposer,
    private readonly policy: HorizonPolicy,
    private readonly runner: (w: WorldComposer, r: ReachRequest, h?: OrchestrationHooks) => Promise<ReachResult> = requestReachAsync,
  ) {}

  /** Note the player's position; prefetch up to `ahead` next Reaches, evict behind. */
  async noteAt(reachIndex: number): Promise<void> {
    for (let k = 1; k <= this.policy.ahead; k++) {
      const idx = reachIndex + k;
      if (this.world.drawnLength !== undefined && idx >= this.world.drawnLength) break;
      if (this.world.realized.has(idx) || this.cache.has(idx)) continue;
      const result = await this.runner(this.world, this.policy.requestFor(idx));
      this.cache.set(idx, result);
    }
    if (this.policy.evictBehind !== undefined) {
      for (const k of [...this.cache.keys()]) if (k < reachIndex - this.policy.evictBehind) this.cache.delete(k);
    }
  }

  /** A prefetched (or already-realized) Reach, if available. */
  get(idx: number): ReachResult | undefined {
    return this.cache.get(idx) ?? this.world.realized.get(idx);
  }

  get cachedIndices(): number[] {
    return [...this.cache.keys()].sort((a, b) => a - b);
  }
}
