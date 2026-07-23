/**
 * Named lock/puzzle builders for the common taxonomy patterns — the authoring
 * convenience over `PuzzleDef`. Hosts (and the shipped presets) declare locks by
 * pattern instead of hand-assembling defs.
 */

import { have, count, flag, and, type CapabilityId, type Rule } from "../logic/index.js";
import type { PuzzleDef } from "./puzzle-def.js";

const power = (w: number) => (): number => w;

/** #1 — a plain capability lock on an edge. */
export function capabilityLock(id: string, cap: CapabilityId, edgeTo: string, opts?: { powerWeight?: number }): PuzzleDef {
  return {
    id,
    scope: "room",
    class: "required",
    condition: have(cap),
    outcome: { kind: "open-edge", edge: { to: edgeTo } },
    powerWeight: power(opts?.powerWeight ?? 0.4),
  };
}

/** #2 — a single physical switch. */
export function switchLock(id: string, flagName: string, edgeTo: string): PuzzleDef {
  return {
    id,
    scope: "room",
    class: "required",
    condition: flag(flagName),
    outcome: { kind: "open-edge", edge: { to: edgeTo } },
    powerWeight: power(0.2),
  };
}

/** #4 — all-on-at-once panel array. */
export function panelArray(id: string, flags: string[], edgeTo: string): PuzzleDef {
  return {
    id,
    scope: "area",
    class: "required",
    condition: and(...flags.map((f) => flag(f))),
    outcome: { kind: "open-edge", edge: { to: edgeTo } },
    powerWeight: power(0.5),
    spatialRecipe: "panel-array",
  };
}

/** #8 — arena lockdown (exit opens once cleared). */
export function arenaLockdown(id: string, clearedFlag: string, edgeTo: string): PuzzleDef {
  return {
    id,
    scope: "room",
    class: "required",
    condition: flag(clearedFlag),
    outcome: { kind: "open-edge", edge: { to: edgeTo } },
    powerWeight: power(0.6),
    spatialRecipe: "arena",
  };
}

/** #13 — a world-scope collectathon shrine. */
export function collectathon(id: string, cap: CapabilityId, n: number, edgeTo: string, opts?: { guaranteeWithin?: number }): PuzzleDef {
  const def: PuzzleDef = {
    id,
    scope: "world",
    class: "required",
    condition: count(cap, n),
    outcome: { kind: "open-edge", edge: { to: edgeTo } },
    powerWeight: power(0.95),
    spatialRecipe: "collectathon-shrine",
  };
  if (opts?.guaranteeWithin !== undefined) def.guarantee = { withinReachLevels: opts.guaranteeWithin };
  return def;
}

/** #12 — a boss gate (cleared flag opens the way onward). */
export function bossGate(id: string, clearedFlag: string, edgeTo: string): PuzzleDef {
  return {
    id,
    scope: "room",
    class: "required",
    condition: flag(clearedFlag),
    outcome: { kind: "open-edge", edge: { to: edgeTo } },
    powerWeight: power(0.9),
    spatialRecipe: "boss-arena",
  };
}

/** A generic bonus/secret reward puzzle (never gates progress). */
export function bonusReward(id: string, condition: Rule, itemId: string): PuzzleDef {
  return {
    id,
    scope: "room",
    class: "optional-reward",
    condition,
    outcome: { kind: "spawn-item-here", item: { itemId } },
    powerWeight: power(0.3),
  };
}
