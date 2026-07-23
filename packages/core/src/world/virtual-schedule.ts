/**
 * The virtual schedule — a one-time, pure plan of roughly which Reach each
 * schedulable-pool entry should land in, spread across `L` Reaches (low-power
 * early, high-power late). Generic over the pool; run independently per pool with
 * its own fork namespace so the pools never share or compete for entropy.
 *
 * Purity is load-bearing: it is a function of `(seed, L, pool, namespace)` only —
 * it never touches the live world RNG, so consulting it never perturbs generation.
 */

import { Rng } from "../math/index.js";

export interface SchedulableEntry {
  id: string;
  powerWeight: (level: number) => number;
  guarantee?: { withinReachLevels: number };
}

export function computeVirtualSchedule<T extends SchedulableEntry>(
  seed: string,
  L: number,
  pool: readonly T[],
  forkNamespace: string,
): Map<string, number> {
  const plan = new Map<string, number>();
  if (L <= 0 || pool.length === 0) return plan;
  const rng = new Rng(seed).fork(forkNamespace);
  const sorted = [...pool].sort((a, b) => a.powerWeight(0) - b.powerWeight(0) || (a.id < b.id ? -1 : 1));
  for (let k = 0; k < sorted.length; k++) {
    const e = sorted[k];
    if (!e) continue;
    const frac = sorted.length <= 1 ? 0 : k / (sorted.length - 1);
    let slot = Math.round(frac * (L - 1)) + rng.int(-1, 1);
    slot = Math.max(0, Math.min(L - 1, slot));
    plan.set(e.id, slot);
  }
  return plan;
}
