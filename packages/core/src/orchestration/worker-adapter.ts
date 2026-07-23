/**
 * Worker seam. A generation job is regenerated from identity `(world, request)` —
 * pure data in, ReachResult out — so it can run inline or (in the Inspector/host)
 * on a Web Worker unchanged, and the byte-identical guarantee is testable by
 * running the same job inline. The real Worker wrapper reconstructs the world from
 * (fingerprint, seed, request) on the far side; here `inlineWorker` runs in-process.
 */

import type { ReachRequest, ReachResult, WorldComposer } from "../world/index.js";
import { requestReachAsync } from "./async.js";

export interface WorkerLike {
  run(request: ReachRequest): Promise<ReachResult>;
}

/** A same-thread WorkerLike — the offload contract, verifiable against the sync core. */
export function inlineWorker(world: WorldComposer): WorkerLike {
  return { run: (request) => requestReachAsync(world, request) };
}
