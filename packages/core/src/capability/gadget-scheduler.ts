/**
 * The gadget scheduler — turns GadgetDefs into schedulable entries (powerWeight =
 * max over granted caps; guarantee = tightest window among them) and draws a
 * per-Reach subset via the shared `scheduleDraw`. Chosen gadgets become
 * progression Items.
 */

import type { CapabilityId } from "../logic/index.js";
import type { CapabilityDef, GadgetDef } from "./capability-def.js";
import type { SchedulableEntry } from "../world/virtual-schedule.js";

export function gadgetEntry(g: GadgetDef, capById: ReadonlyMap<CapabilityId, CapabilityDef>): SchedulableEntry {
  const caps = g.grants.map((id) => capById.get(id)).filter((c): c is CapabilityDef => c !== undefined);
  let tightest: number | undefined;
  for (const c of caps) {
    if (c.guarantee) tightest = tightest === undefined ? c.guarantee.withinReachLevels : Math.min(tightest, c.guarantee.withinReachLevels);
  }
  const entry: SchedulableEntry & { guarantee?: { withinReachLevels: number } } = {
    id: g.id,
    powerWeight: (level: number) => {
      let m = 0;
      for (const c of caps) m = Math.max(m, c.powerWeight(level));
      return m;
    },
  };
  if (tightest !== undefined) entry.guarantee = { withinReachLevels: tightest };
  return entry;
}
