/**
 * The async facade over the sync core. `requestReachAsync` drives the composer's
 * resumable `reachSteps` generator, yielding + checking cancellation + reporting
 * progress at each phase boundary, and committing ONLY after a final cancellation
 * check — so a cancelled run leaves the composer untouched (atomicity) and an
 * uncancelled run is byte-identical to the sync `requestReach` (parity).
 */

import type { ReachRequest, ReachResult, WorldComposer } from "../world/index.js";
import type { DiagnosticsConfig } from "../diagnostics.js";
import type { CancellationToken } from "./cancellation.js";
import { ProgressTracker, type GenProgress } from "./progress.js";

export interface OrchestrationHooks {
  onProgress?: (p: GenProgress) => void;
  /** Host-pure cooperative yield (frame budget). May be sync or async — always awaited. */
  shouldYield?: () => Promise<void> | void;
  token?: CancellationToken;
  /** Per-run diagnostics override (threaded to composers as they gain emit points). */
  diagnostics?: DiagnosticsConfig;
  /** Clock for elapsedMs (facade-only; core never reads a clock). Default Date.now. */
  now?: () => number;
}

export async function requestReachAsync(world: WorldComposer, request: ReachRequest, hooks: OrchestrationHooks = {}): Promise<ReachResult> {
  const now = hooks.now ?? (() => Date.now());
  const start = now();
  const tracker = new ProgressTracker();

  const g = world.reachSteps(request);
  let n = g.next();
  while (!n.done) {
    const phase = n.value;
    hooks.token?.throwIfCancelled();
    await hooks.shouldYield?.();
    hooks.onProgress?.(tracker.complete(phase, now() - start));
    n = g.next();
  }
  // final cancellation check BEFORE commit → atomicity
  hooks.token?.throwIfCancelled();
  n.value.commit();
  return n.value.result;
}
