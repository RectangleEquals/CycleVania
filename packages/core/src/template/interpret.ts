/**
 * Template interpretation — turn a `ReachTemplate` + the Reach's already-selected
 * content into a concrete `MissionGraph`. Selection happens BEFORE interpretation
 * (a gate's rule must reference selected capabilities/puzzles), so this receives
 * `gateRules` to bind onto gate slots in priority order. The step sequence and its
 * RNG draw order are contractual (part of determinism).
 *
 * Steps: instantiate nodes → wire spine → hang branches → collect gate sites
 * (gate-role entrances, then vault entrances, then lockFraction of spine edges) and
 * bind rules → close loops → allocate Location slots → assert the bootstrap
 * invariant → register flag provenance. Never invents a rule; surplus gate slots
 * stay open.
 */

import { ALWAYS, and, ruleFlags, heldOf } from "../logic/index.js";
import type { Rule } from "../logic/index.js";
import { GenError } from "../errors.js";
import { reachableRegions } from "../graph/index.js";
import type { Edge, FlagDef, LocationDef, MissionGraph, Region } from "../graph/index.js";
import type { Rng } from "../math/index.js";
import type { BranchSpec, ReachTemplate } from "./reach-template.js";

export interface SelectedContent {
  /** Rules to bind onto gate slots, in priority order (built by M04's selector). */
  gateRules: Rule[];
  /** For the bootstrap invariant: how many progression items this Reach places. */
  progressionCount: number;
}

export interface StructureNudges {
  extraBranchChance?: number;
  extraLoopChance?: number;
}

const spineIndex = (cp: readonly string[], id: string): number => cp.indexOf(id);

export function interpretTemplate(
  t: ReachTemplate,
  content: SelectedContent,
  nudges: StructureNudges,
  rng: Rng,
): MissionGraph {
  const cp = t.criticalPath;
  const start = cp[0];
  if (start === undefined) throw new GenError("template.empty", `template "${t.id}" has an empty criticalPath`);

  const regions: Region[] = [];
  const edges: Edge[] = [];
  const flags: FlagDef[] = [];
  const locations: LocationDef[] = [];

  // 1. instantiate critical-path nodes
  for (const id of cp) {
    const n = t.nodes[id];
    if (!n) throw new GenError("template.bad-node", `criticalPath references undefined node "${id}" in "${t.id}"`);
    regions.push({ id, role: n.role });
  }

  // 2. spine edges (ALWAYS initially)
  for (let i = 0; i < cp.length - 1; i++) {
    const from = cp[i];
    const to = cp[i + 1];
    if (from === undefined || to === undefined) continue;
    edges.push({ from, to, rule: ALWAYS });
  }

  // 3. branches (vaults)
  let vaultN = 0;
  const vaultEntrances: Edge[] = [];
  const addBranch = (b: BranchSpec): void => {
    if (spineIndex(cp, b.attachTo) < 0) return; // attach point must be on the spine
    const vid = `${b.attachTo}~${b.role}${vaultN++}`;
    regions.push({ id: vid, role: b.role });
    const entrance: Edge = { from: b.attachTo, to: vid, rule: ALWAYS };
    edges.push(entrance);
    if (b.entrance !== "optional-open") vaultEntrances.push(entrance);
    const slots = rng.int(b.slots.min, b.slots.max);
    for (let k = 0; k < slots; k++) locations.push({ id: `${vid}#${k}`, region: vid });
    if (rng.chance(b.backEdgeChance + (nudges.extraLoopChance ?? 0))) {
      const ai = spineIndex(cp, b.attachTo);
      const earlier = cp[rng.int(0, Math.max(0, ai - 1))];
      if (earlier !== undefined && earlier !== vid) edges.push({ from: vid, to: earlier, rule: ALWAYS });
    }
  };
  for (const b of t.branches) {
    addBranch(b);
    if (rng.chance(nudges.extraBranchChance ?? 0)) addBranch(b);
  }

  // 4. gate sites, in priority order, then bind rules verbatim (never invented)
  const isSpineEdge = (e: Edge): boolean => {
    const fi = spineIndex(cp, e.from);
    const ti = spineIndex(cp, e.to);
    return fi >= 0 && ti === fi + 1;
  };
  const gateSites: Edge[] = [];
  for (const e of edges) if (isSpineEdge(e) && t.nodes[e.to]?.role === "gate") gateSites.push(e);
  for (const e of vaultEntrances) gateSites.push(e);
  const lockable = edges.filter((e) => {
    if (!isSpineEdge(e) || gateSites.includes(e)) return false;
    const fi = spineIndex(cp, e.from);
    if (t.gating.keepEntryOpen && fi === 0) return false;
    if (t.gating.keepExitOpen && fi === cp.length - 2) return false;
    return true;
  });
  const lockCount = Math.round(t.gating.lockFraction * lockable.length);
  const shuffledLockable = rng.shuffle(lockable);
  for (let i = 0; i < lockCount; i++) {
    const e = shuffledLockable[i];
    if (e) gateSites.push(e);
  }

  let ri = 0;
  for (const site of gateSites) {
    const r0 = content.gateRules[ri];
    if (r0 === undefined) break; // surplus gate slots stay ALWAYS — never invent a rule
    const r1 = content.gateRules[ri + 1];
    if (r1 !== undefined && rng.chance(t.gating.compoundChance)) {
      site.rule = and(r0, r1);
      ri += 2;
    } else {
      site.rule = r0;
      ri += 1;
    }
  }

  // 5. loops
  const backEdgeCount = (): number =>
    edges.filter((e) => {
      const fi = spineIndex(cp, e.from);
      const ti = spineIndex(cp, e.to);
      return fi >= 0 && ti >= 0 && ti < fi;
    }).length;
  if (t.loops.guaranteeAtLeastOne && backEdgeCount() === 0 && cp.length >= 3) {
    const from = cp[cp.length - 2];
    const to = cp[1];
    if (from !== undefined && to !== undefined) edges.push({ from, to, rule: ALWAYS });
  }
  if (cp.length >= 3) {
    const extraAttempts = Math.round(t.loops.density * cp.length);
    for (let a = 0; a < extraAttempts; a++) {
      if (!rng.chance(t.loops.density + (nudges.extraLoopChance ?? 0))) continue;
      const fi = rng.int(2, cp.length - 1);
      const ti = rng.int(1, fi - 1);
      const from = cp[fi];
      const to = cp[ti];
      if (from !== undefined && to !== undefined) edges.push({ from, to, rule: ALWAYS });
    }
  }

  // 6. Location slots per critical-path node
  for (const id of cp) {
    const n = t.nodes[id];
    if (!n) continue;
    const slots = rng.int(n.slots.min, n.slots.max);
    for (let k = 0; k < slots; k++) locations.push({ id: `${id}#${k}`, region: id });
  }

  const graph: MissionGraph = { regions, edges, flags, locations, start };

  // 7. bootstrap invariant — always-reachable, non-gated slots ≥ progressionCount + 1
  const openRegions = reachableRegions(graph, heldOf([]));
  const openSlots = locations.filter((l) => openRegions.has(l.region) && l.gate === undefined).length;
  if (openSlots < content.progressionCount + 1) {
    throw new GenError(
      "template.bootstrap",
      `template "${t.id}" provisions ${openSlots} always-reachable slot(s) but needs ≥ ` +
        `${content.progressionCount + 1} for ${content.progressionCount} progression item(s)`,
      { openSlots, need: content.progressionCount + 1 },
    );
  }

  // 8. flag provenance — register a setter for any bound flag with none
  //    (crude default; puzzle outcomes refine `setBy` in M04)
  const referenced = new Set<string>();
  for (const e of edges) for (const f of ruleFlags(e.rule)) referenced.add(f);
  if (referenced.size > 0) {
    const startLoc = locations.find((l) => l.region === start) ?? locations[0];
    for (const name of referenced) {
      if (!flags.some((f) => f.name === name) && startLoc) flags.push({ name, setBy: startLoc.id });
    }
  }

  return graph;
}
