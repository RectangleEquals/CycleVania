/**
 * The `ReachRequest` — the on-demand generation unit. `requestIdentity` is the
 * canonical hash used BOTH as the Reach's root RNG fork label and as the recorded
 * reproducibility key: same identity ⇒ byte-identical Reach.
 */

import type { ReachTemplate } from "../template/index.js";
import type { ReachModifierId } from "./modifiers.js";

export interface ReachRequest {
  reachIndex: number;
  fromReachIndex?: number; // the already-realized Reach this originated from (undefined only for 0)
  chosenModifiers: ReachModifierId[];
  template?: ReachTemplate; // optional override of the pool draw
  gadgetEconomyOverride?: Partial<{ min: number; max: number }>;
  puzzleEconomyOverride?: Partial<{ min: number; max: number }>;
}

/** A stored request record (for the reproducibility log). */
export interface ReachRequestRecord {
  request: ReachRequest;
  identity: string;
}

function stableEcon(o?: Partial<{ min: number; max: number }>): string {
  if (!o) return "";
  const parts: string[] = [];
  if (o.min !== undefined) parts.push(`min:${o.min}`);
  if (o.max !== undefined) parts.push(`max:${o.max}`);
  return parts.join(",");
}

/** The canonical identity of a request — reach index + everything that varies the outcome. */
export function requestIdentity(r: ReachRequest): string {
  const mods = [...r.chosenModifiers].sort().join(",");
  return (
    `reach${r.reachIndex}` +
    `:mods[${mods}]` +
    `:econ[${stableEcon(r.gadgetEconomyOverride)}]` +
    `:pecon[${stableEcon(r.puzzleEconomyOverride)}]` +
    `:tpl[${r.template?.id ?? "pool"}]`
  );
}
