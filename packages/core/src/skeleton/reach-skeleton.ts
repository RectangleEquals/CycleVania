/**
 * The ReachComposer's L2 pass — one Area per Region, laid out relative to each
 * other (Areas as nodes, L1 edges as springs, one-way drops pulling targets
 * lower), then inter-area connectors wired between boundary sockets carrying each
 * L1 edge's gate BY REFERENCE. Landmarks (1–2 Spaces per Reach) are chosen here.
 */

import { Rng, add, boxUnionAll, type Vec3 } from "../math/index.js";
import type { MissionGraph, RegionId } from "../graph/index.js";
import type { PuzzleInstance } from "../puzzle/puzzle-def.js";
import { DEFAULT_AREA_DIALS, splitReachBudget, type AreaDialConfig } from "./space-budget.js";
import { composeArea } from "./area-composer.js";
import { forceLayout, type LayoutEdge, type LayoutNode } from "./force-layout.js";
import type { AreaSkeleton, ConnectorSpec, ProvisionalSocket, ReachSkeleton, SocketRef, SpaceSpec } from "./space-plan.js";

export interface BuildReachSkeletonOptions {
  finalCeiling: number;
  buckets: Readonly<Record<string, number>>;
  biome?: string;
  puzzleInstances?: PuzzleInstance[];
  dialCfg?: AreaDialConfig;
  landmarksPerReach?: { min: number; max: number };
}

function offsetSocket(s: ProvisionalSocket, o: Vec3): void {
  s.pos = add(s.pos, o);
}
function offsetSpace(s: SpaceSpec, o: Vec3): void {
  s.origin = add(s.origin, o);
  s.envelope = { min: add(s.envelope.min, o), max: add(s.envelope.max, o) };
}
function offsetConnector(c: ConnectorSpec, o: Vec3): void {
  if (c.waypoints) c.waypoints = c.waypoints.map((w) => add(w, o));
}

export function buildReachSkeleton(graph: MissionGraph, seed: string, opts: BuildReachSkeletonOptions): ReachSkeleton {
  const rng = new Rng(seed);
  const dialCfg = opts.dialCfg ?? DEFAULT_AREA_DIALS;
  const biome = opts.biome ?? "default";
  const roles = graph.regions.map((r) => r.role);
  const slices = splitReachBudget(opts.finalCeiling, roles, rng.fork("budget"));

  // incident edge count + reserved recipes per region
  const incident = new Map<RegionId, number>();
  for (const r of graph.regions) incident.set(r.id, 0);
  for (const e of graph.edges) {
    incident.set(e.from, (incident.get(e.from) ?? 0) + 1);
    incident.set(e.to, (incident.get(e.to) ?? 0) + 1);
  }
  const reservedByRegion = new Map<RegionId, string[]>();
  for (const inst of opts.puzzleInstances ?? []) {
    if (!inst.spatialRecipe) continue;
    // the region behind the edge this puzzle's condition gates (shared reference)
    const edge = graph.edges.find((e) => e.rule === inst.condition);
    const region = edge?.to ?? graph.regions.find((r) => r.role === "capstone")?.id ?? graph.start;
    const list = reservedByRegion.get(region) ?? [];
    list.push(inst.spatialRecipe);
    reservedByRegion.set(region, list);
  }

  // --- compose each Area at local origin ---
  const areas: AreaSkeleton[] = graph.regions.map((r, i) =>
    composeArea({
      regionId: r.id,
      role: r.role,
      budgetSlice: slices[i] ?? 60,
      buckets: opts.buckets,
      biome,
      incidentEdges: incident.get(r.id) ?? 0,
      reservedRecipes: reservedByRegion.get(r.id) ?? [],
      dialCfg,
      rng: rng.fork(`area:${r.id}`),
    }),
  );
  const areaById = new Map(areas.map((a) => [a.regionId, a] as const));

  // --- reach-level area layout (Areas as nodes, L1 edges as springs) ---
  const areaRadius = (a: AreaSkeleton): number => {
    const box = boxUnionAll(a.spaces.map((s) => s.envelope));
    return Math.max(6, (box.max[0] - box.min[0] + box.max[1] - box.min[1]) / 2);
  };
  // z-plan: one-way (drop) L1 edges pull the target Area lower
  const targetZ = new Map<RegionId, number>([[graph.start, 0]]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const e of graph.edges) {
      const fz = targetZ.get(e.from);
      if (fz === undefined) continue;
      const want = e.oneWay ? fz - Math.max(8, opts.finalCeiling / 12) : fz;
      const cur = targetZ.get(e.to);
      if (cur === undefined || (e.oneWay && want < cur)) {
        targetZ.set(e.to, want);
        changed = true;
      }
    }
  }
  const nodes: LayoutNode[] = areas.map((a, i) => ({
    id: a.regionId,
    radius: areaRadius(a),
    ...(i === 0 ? { pinned: [0, 0, 0] as Vec3 } : {}),
    targetZ: targetZ.get(a.regionId) ?? 0,
  }));
  const edges: LayoutEdge[] = graph.edges.map((e) => ({ a: e.from, b: e.to }));
  const { positions } = forceLayout(nodes, edges, { seed: `${seed}:areas`, iterations: 260, spread: 40, zSeparation: 1.6, zPull: 0.4 });

  // translate each Area into reach space
  const areaOrigins: Record<RegionId, Vec3> = {};
  for (const a of areas) {
    const o = positions.get(a.regionId) ?? [0, 0, 0];
    areaOrigins[a.regionId] = o;
    for (const s of a.spaces) offsetSpace(s, o);
    for (const c of a.connectors) offsetConnector(c, o);
    for (const sk of a.sockets) offsetSocket(sk, o); // boundarySockets are a subset (same refs)
  }

  // --- inter-area connectors: one per L1 edge, gate BY REFERENCE ---
  const interAreaConnectors: ConnectorSpec[] = [];
  const relaxations: string[] = [];
  const nextBoundary = new Map<RegionId, number>();
  const takeSocket = (region: RegionId): ProvisionalSocket | undefined => {
    const a = areaById.get(region);
    if (!a) return undefined;
    const idx = nextBoundary.get(region) ?? 0;
    nextBoundary.set(region, idx + 1);
    return a.boundarySockets[idx];
  };
  let ci = 0;
  for (const e of graph.edges) {
    const sa = takeSocket(e.from);
    const sb = takeSocket(e.to);
    if (!sa || !sb) {
      relaxations.push("skeleton.inter-area-socket-missing");
      continue;
    }
    const from: SocketRef = { spaceId: sa.spaceId, socketId: sa.id };
    const to: SocketRef = { spaceId: sb.spaceId, socketId: sb.id };
    const conn: ConnectorSpec = {
      id: `inter:${ci++}`,
      from,
      to,
      kind: e.oneWay ? "shaft" : "straight",
      traversal: e.oneWay ? "drop" : "walk",
      lengthBounds: { min: 4, max: 60 },
    };
    if (e.rule.k !== "always") conn.gate = e.rule; // BY REFERENCE
    if (e.oneWay) conn.oneWay = true;
    interAreaConnectors.push(conn);
  }

  // --- landmarks: 1–2 Spaces per Reach, biased to hub-role Areas ---
  const lmRange = opts.landmarksPerReach ?? { min: 1, max: 2 };
  const lmCount = Math.min(rng.fork("lm").int(lmRange.min, lmRange.max), areas.length);
  const candidates = areas
    .flatMap((a) => a.spaces.map((s) => ({ s, hub: a.role === "hub" })))
    .sort((x, y) => (x.hub === y.hub ? 0 : x.hub ? -1 : 1));
  const landmarkSpaceIds: string[] = [];
  for (let k = 0; k < lmCount && k < candidates.length; k++) {
    const c = candidates[k];
    if (c) {
      c.s.landmark = true;
      landmarkSpaceIds.push(c.s.id);
    }
  }

  return { areas, areaOrigins, interAreaConnectors, landmarkSpaceIds, relaxations: [...relaxations] };
}
