/**
 * autosolve — the bot-completes-a-Reach proof. A sphere-guided greedy walk:
 * collect every reachable Location (folding grants + non-volatile flags), solve
 * every satisfiable puzzle, expand reachability, until the terminal is reachable.
 * If it ever gets stuck though `isSolvable` passed at generation, that's a
 * generator bug — throw loudly (the two proofs disagreeing is the highest-value
 * signal this harness exists to catch).
 */

import { evalRule } from "../logic/index.js";
import { initSim, type SimState } from "./state.js";
import { reachableNodes, type SimWorld } from "./world.js";

export interface AutosolveResult {
  state: SimState;
  success: boolean;
  /** Locations collected per progression reward — the discovery-drought pacing metric. */
  movesBetweenRewards: number;
}

export function autosolve(world: SimWorld): AutosolveResult {
  const state = initSim(world);
  let collectedCount = 0;
  let rewards = 0;

  for (let iter = 0; iter < 4000; iter++) {
    const reachable = reachableNodes(world, state.held);
    if (world.terminal !== undefined && reachable.has(world.terminal)) {
      state.at = world.terminal;
      state.log.push("reach terminal");
      return { state, success: true, movesBetweenRewards: collectedCount / Math.max(1, rewards) };
    }
    let progress = false;
    // collect (lowest sphere first for a plausible order)
    const collectable = [...reachable]
      .flatMap((rid) => world.nodes.get(rid)?.locations ?? [])
      .filter((loc) => !state.collected.has(loc.id))
      .sort((a, b) => (a.sphere ?? 999) - (b.sphere ?? 999));
    for (const loc of collectable) {
      state.collected.add(loc.id);
      collectedCount++;
      const item = loc.itemId !== undefined ? world.items.get(loc.itemId) : undefined;
      if (item && item.grants.length > 0) {
        for (const c of item.grants) state.held.add(c);
        rewards++;
      }
      for (const f of world.flagSetters.get(loc.id) ?? []) state.held.addFlag(f);
      progress = true;
    }
    // solve satisfiable puzzles
    for (const p of world.puzzles) {
      if (state.solvedPuzzles.has(p.instanceId) || !evalRule(p.condition, state.held)) continue;
      state.solvedPuzzles.add(p.instanceId);
      state.held.addFlag(p.instanceId);
      if (p.outcome.kind === "grant-capability") state.held.add(p.outcome.capability);
      progress = true;
    }
    if (!progress) break;
  }

  // final check
  if (world.terminal !== undefined && reachableNodes(world, state.held).has(world.terminal)) {
    state.at = world.terminal;
    return { state, success: true, movesBetweenRewards: collectedCount / Math.max(1, rewards) };
  }
  if (world.terminal === undefined) {
    const allNonBonus = [...world.nodes.values()].flatMap((n) => n.locations).filter((l) => !l.bonus).every((l) => state.collected.has(l.id));
    if (allNonBonus) return { state, success: true, movesBetweenRewards: collectedCount / Math.max(1, rewards) };
  }
  throw new Error(`autosolve stuck: terminal unreachable though the Reach was proven solvable (collected ${state.collected.size} Locations, held ${state.held.capIds().join(",")})`);
}
