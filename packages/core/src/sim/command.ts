/**
 * Simulator commands — the interactive vocabulary the inspector REPL drives
 * (`/goto`, `/use`, `/take`, `/give`, `/why`, `/reset`, `/solve`).
 */

import type { Capability } from "../logic/index.js";

export type Command =
  | { k: "goto"; areaId: number }
  | { k: "use"; itemId: string }
  | { k: "take" }
  | { k: "give"; cap: Capability }
  | { k: "why"; areaId: number }
  | { k: "reset" }
  | { k: "solve" };
