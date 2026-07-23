/**
 * Example puzzle catalog. These are OPTIONAL-class (reward / shortcut) so they
 * enrich a world without ever gating the critical path — the selector's own
 * teach→test→combine capability gates drive required progression. A host authors
 * required puzzles (and pins world-scope set-pieces by template role) for its own
 * game; the Metroid Prime fixture set shows the required patterns.
 */

import { have, flag, type PuzzleDef } from "@cyclevania/core";

export const PUZZLES: PuzzleDef[] = [
  {
    id: "secret-vault",
    scope: "room",
    class: "optional-reward",
    condition: have("reveal"),
    outcome: { kind: "spawn-item-here", item: { itemId: "the-leap" } },
    powerWeight: () => 0.3,
    spatialRecipe: "hidden-crossing",
    archetype: "perception",
  },
  {
    id: "grapple-shortcut",
    scope: "area",
    class: "optional-shortcut",
    condition: have("grapple"),
    outcome: { kind: "set-flag-only" },
    powerWeight: () => 0.3,
    archetype: "shortcut",
  },
  {
    id: "arena-trial",
    scope: "room",
    class: "optional-reward",
    condition: flag("arena-cleared"),
    outcome: { kind: "set-flag-only" },
    powerWeight: () => 0.5,
    spatialRecipe: "arena",
    archetype: "arena",
  },
  {
    id: "torch-puzzle",
    scope: "room",
    class: "optional-reward",
    condition: have("ignite"),
    outcome: { kind: "spawn-item-here", item: { itemId: "the-charge" } },
    powerWeight: () => 0.4,
    spatialRecipe: "panel-array",
    archetype: "torch-puzzle",
  },
];
