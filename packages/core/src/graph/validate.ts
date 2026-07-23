/**
 * Construction-time precondition — fail loudly, never silently. Every Region must
 * be reachable when the party holds `fullyEquippedHeld` (startHeld + this Reach's
 * own placements + its settable non-volatile flags). If not, the template/
 * embedding is malformed — throw a `GenError` naming the stranded Regions and, for
 * each, the incoming edges whose rule was the last unsatisfiable frontier. Also
 * asserts flag provenance: every flag any rule references must have a setter.
 */

import { evalRule, ruleFlags, missingCaps, type Held } from "../logic/index.js";
import { GenError } from "../errors.js";
import type { Diag } from "../diagnostics.js";
import { reachableRegions } from "./reachability.js";
import type { MissionGraph, RegionId } from "./mission-graph.js";

export function validateGraph(g: MissionGraph, fullyEquippedHeld: Held, diag?: Diag): void {
  // --- flag provenance ---
  const referenced = new Set<string>();
  for (const e of g.edges) for (const f of ruleFlags(e.rule)) referenced.add(f);
  for (const loc of g.locations) if (loc.gate) for (const f of ruleFlags(loc.gate)) referenced.add(f);
  const declared = new Set(g.flags.map((f) => f.name));
  const locationIds = new Set(g.locations.map((l) => l.id));
  const orphanFlags: string[] = [];
  for (const name of referenced) {
    const def = g.flags.find((f) => f.name === name);
    if (!declared.has(name) || !def || !locationIds.has(def.setBy)) orphanFlags.push(name);
  }
  if (orphanFlags.length > 0) {
    diag?.error("graph.unset-flag", `flag(s) referenced with no reachable setter: ${orphanFlags.join(", ")}`);
    throw new GenError("graph.unset-flag", `Rule references flag(s) with no valid setter: ${orphanFlags.join(", ")}`, {
      flags: orphanFlags,
    });
  }

  // --- reachability when fully equipped ---
  const reached = reachableRegions(g, fullyEquippedHeld);
  const stranded = g.regions.map((r) => r.id).filter((id) => !reached.has(id));
  if (stranded.length === 0) return;

  const frontier: Record<RegionId, { from: RegionId; needs: string[] }[]> = {};
  const allNeeds = new Set<string>();
  for (const id of stranded) {
    const blocking = g.edges
      .filter((e) => e.to === id && reached.has(e.from) && !evalRule(e.rule, fullyEquippedHeld))
      .map((e) => {
        const needs = missingCaps(e.rule, fullyEquippedHeld);
        for (const c of needs) allNeeds.add(c);
        return { from: e.from, needs };
      });
    frontier[id] = blocking;
  }
  const needsStr = allNeeds.size > 0 ? ` (missing capabilities: ${[...allNeeds].join(", ")})` : "";
  diag?.error("graph.stranded-region", `unreachable region(s) when fully equipped: ${stranded.join(", ")}${needsStr}`);
  throw new GenError(
    "graph.stranded-region",
    `Region(s) unreachable when fully equipped: ${stranded.join(", ")}${needsStr}. ` +
      `This is a template/embedding bug — a Region gated behind an out-of-scope or dangling edge.`,
    { stranded, frontier },
  );
}
