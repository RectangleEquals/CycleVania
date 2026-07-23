/** A `PuzzleDef` is a `SchedulableEntry` directly; this maps to the schedule shape. */

import type { SchedulableEntry } from "../world/virtual-schedule.js";
import type { PuzzleDef } from "./puzzle-def.js";

export function puzzleEntry(def: PuzzleDef): SchedulableEntry {
  const e: SchedulableEntry = { id: def.id, powerWeight: def.powerWeight };
  if (def.guarantee) e.guarantee = def.guarantee;
  return e;
}
