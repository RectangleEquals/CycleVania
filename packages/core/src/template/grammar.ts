/**
 * Reach grammar — interprets a `ReachTemplate` into a `RegionGraph` + a
 * guaranteed-solvable progression `Placement`. The template supplies the macro
 * shape (roles, slots, gating, branches, loops); the item catalog supplies the
 * concrete capabilities. Three invariants keep it softlock-impossible for ANY
 * well-formed template:
 *   1. bootstrap nodes over-provision ≥ items+1 always-reachable slots;
 *   2. `validateGraph` asserts every region is reachable fully-equipped;
 *   3. `assumedFill` + its `isSolvable` regression check close the loop.
 * A failure here signals a malformed template, not bad runtime input — so we
 * throw loudly rather than ship a broken world.
 */

import { Rng, shuffle } from "../math/rng.js";
import { ALWAYS, and, have, type Capability, type Rule } from "../logic/index.js";
import { assumedFill } from "../fill/assumed-fill.js";
import { validateGraph } from "../graph/solvable.js";
import type {
  LocationId,
  Placement,
  ProgressionItem,
  RegionEdge,
  RegionGraph,
  RegionId,
} from "../graph/region-graph.js";
import type { RegionRole } from "./role.js";
import type { ReachTemplate, TemplateNode } from "./template.js";

export interface GenerateReachParams {
  seed: string | number;
  template: ReachTemplate;
  /** Progression items to place & to gate with. */
  items: readonly ProgressionItem[];
  /** Capabilities the party already holds entering the Reach (usually none). */
  startCaps?: Iterable<Capability>;
}

export interface ReachMeta {
  hub: RegionId;
  spine: RegionId[]; // critical-path segment regions, in order
  capstone: RegionId | null;
  terminal: RegionId | null;
  vaults: RegionId[];
  gatedEdges: RegionEdge[];
  cycleEdges: RegionEdge[];
  roles: Map<RegionId, RegionRole>;
}

export interface GeneratedReach {
  graph: RegionGraph;
  items: ProgressionItem[];
  placement: Placement;
  startCaps: Set<Capability>;
  meta: ReachMeta;
}

export function generateReach(params: GenerateReachParams): GeneratedReach {
  const { template } = params;
  const rng = new Rng(params.seed);
  const items = params.items.map((g) => ({ ...g }));
  const startCaps = new Set<Capability>(params.startCaps ?? []);
  const lockCaps = items.map((i) => i.grants).filter((c) => !startCaps.has(c));

  const regions = new Set<RegionId>();
  const edges: RegionEdge[] = [];
  const locations = new Map<LocationId, RegionId>();
  const gatedEdges: RegionEdge[] = [];
  const cycleEdges: RegionEdge[] = [];
  const roles = new Map<RegionId, RegionRole>();
  let locN = 0;

  const node = (id: string): TemplateNode => {
    const n = template.nodes[id];
    if (!n) throw new Error(`generateReach: criticalPath node "${id}" missing from template.nodes`);
    return n;
  };
  const addLoc = (region: RegionId, tag: string): void => {
    locations.set(`L${locN++}.${region}.${tag}`, region);
  };
  const addEdge = (from: RegionId, to: RegionId, rule: Rule): RegionEdge => {
    const e: RegionEdge = { from, to, rule };
    edges.push(e);
    if (rule.k !== "always") gatedEdges.push(e);
    return e;
  };

  // --- Critical-path regions + their location slots ---
  const path = template.criticalPath;
  if (path.length < 2) throw new Error("generateReach: criticalPath needs ≥ 2 nodes");
  for (const id of path) {
    const n = node(id);
    regions.add(id);
    roles.set(id, n.role);
    const slotCount = n.bootstrap
      ? items.length + 1 + rng.int(0, 1)
      : rng.int(n.slots.min, n.slots.max);
    for (let j = 0; j < slotCount; j++) addLoc(id, `slot${j}`);
  }

  const hub = path[0] as RegionId;
  const spine = path.filter((id) => node(id).role === "segment") as RegionId[];
  const capstone = (path.find((id) => node(id).role === "capstone") ?? null) as RegionId | null;
  const terminal = (path.find((id) => node(id).role === "terminal") ?? null) as RegionId | null;

  // --- Gate a fraction of the internal critical-path edges ---
  const gateableIdx: number[] = [];
  for (let i = 0; i < path.length - 1; i++) {
    if (template.gating.keepEntryOpen && i === 0) continue;
    if (template.gating.keepExitOpen && i === path.length - 2) continue;
    gateableIdx.push(i);
  }
  const gateAt = new Map<number, Capability>();
  if (lockCaps.length > 0 && template.gating.lockFraction > 0) {
    const shuffled = shuffle(gateableIdx.slice(), rng);
    const gateCount = Math.min(shuffled.length, Math.max(1, Math.ceil(shuffled.length * template.gating.lockFraction)));
    for (let g = 0; g < gateCount; g++) {
      const idx = shuffled[g];
      if (idx === undefined) break;
      gateAt.set(idx, lockCaps[g % lockCaps.length] as Capability);
    }
  }
  for (let i = 0; i < path.length - 1; i++) {
    const from = path[i] as RegionId;
    const to = path[i + 1] as RegionId;
    // a node explicitly roled "gate" always locks its entrance
    let cap = gateAt.get(i);
    if (!cap && node(to).role === "gate" && lockCaps.length > 0) cap = rng.pick(lockCaps);
    addEdge(from, to, cap ? have(cap) : ALWAYS);
  }

  // --- Branch vaults + loop shortcuts ---
  const vaults: RegionId[] = [];
  let vaultN = 0;
  for (const branch of template.branches) {
    const anchors =
      branch.anchor === "any-segment"
        ? spine
        : (regions.has(branch.anchor) ? [branch.anchor] : spine);
    if (anchors.length === 0) continue;
    const count = branch.count
      ? rng.int(branch.count.min, branch.count.max)
      : Math.max(1, Math.min(2, lockCaps.length || 1));
    for (let p = 0; p < count; p++) {
      const anchorIdx = rng.int(0, anchors.length - 1);
      const anchor = anchors[anchorIdx] as RegionId;
      const vault = `vault${++vaultN}`;
      regions.add(vault);
      roles.set(vault, "vault");
      vaults.push(vault);
      const nloc = rng.int(branch.slots.min, branch.slots.max);
      for (let j = 0; j < nloc; j++) addLoc(vault, `cache${j}`);

      let entrance: Rule = ALWAYS;
      if (branch.entrance === "compound" && lockCaps.length >= 2 && rng.chance(template.gating.compoundChance)) {
        const two = shuffle(lockCaps.slice(), rng);
        entrance = and(have(two[0] as Capability), have(two[1] as Capability));
      } else if (branch.entrance !== "optional-open" && lockCaps.length > 0) {
        entrance = have(rng.pick(lockCaps));
      }
      addEdge(anchor, vault, entrance);

      // back-edge to an earlier segment → a loop / shortcut
      const spineAnchorIdx = spine.indexOf(anchor);
      if (branch.backEdge && spineAnchorIdx > 0 && rng.chance(branch.backEdge.chance)) {
        const target = spine[rng.int(0, spineAnchorIdx - 1)] as RegionId;
        const backCap = branch.backEdge.gated > 0 && lockCaps.length > 0 && rng.chance(branch.backEdge.gated) ? rng.pick(lockCaps) : undefined;
        cycleEdges.push(addEdge(vault, target, backCap ? have(backCap) : ALWAYS));
      }
    }
  }

  // guarantee at least one real cycle (backtracking structure)
  if (template.loops.guaranteeAtLeastOne && cycleEdges.length === 0 && vaults.length > 0 && spine.length >= 1) {
    cycleEdges.push(addEdge(vaults[0] as RegionId, spine[0] as RegionId, ALWAYS));
  }

  const graph: RegionGraph = { start: hub, regions, edges, locations };

  const v = validateGraph(graph);
  if (!v.ok) {
    throw new Error(`generateReach: graph has stranded regions [${v.stranded.join(", ")}] — malformed template`);
  }

  const placement = assumedFill(graph, items, startCaps, rng.fork("fill"));
  if (!placement) {
    throw new Error("generateReach: assumedFill failed — malformed template (should be impossible by construction)");
  }

  return {
    graph,
    items,
    placement,
    startCaps,
    meta: { hub, spine, capstone, terminal, vaults, gatedEdges, cycleEdges, roles },
  };
}
