/**
 * Puzzle instantiation + outcome introspection. `instantiate` shares the def's
 * `condition` Rule BY REFERENCE (the coupling invariant: one object, one source of
 * truth). The outcome helpers expose what a solved puzzle contributes to
 * solvability (a granted capability, a set flag) so the selector can fold it into
 * the graph.
 */

import type { CapabilityId } from "../logic/index.js";
import type { PuzzleDef, PuzzleInstance, PuzzleOutcome } from "./puzzle-def.js";

export function instantiate(def: PuzzleDef, instanceId: string): PuzzleInstance {
  const inst: PuzzleInstance = {
    instanceId,
    defId: def.id,
    scope: def.scope,
    class: def.class,
    condition: def.condition,
    outcome: def.outcome,
  };
  if (def.spatialRecipe !== undefined) inst.spatialRecipe = def.spatialRecipe;
  return inst;
}

/** The capability a solved puzzle grants, if any. */
export function outcomeGrantsCapability(o: PuzzleOutcome): CapabilityId | undefined {
  return o.kind === "grant-capability" ? o.capability : undefined;
}

/** True when solving the puzzle sets a world flag (its own name = the instance id). */
export function outcomeSetsFlag(o: PuzzleOutcome): boolean {
  return o.kind === "set-flag-only" || o.kind === "open-edge";
}

/** The edge a solved puzzle opens, if any. */
export function outcomeOpensEdge(o: PuzzleOutcome): { from?: string; to: string; oneWay?: boolean } | undefined {
  return o.kind === "open-edge" ? o.edge : undefined;
}
