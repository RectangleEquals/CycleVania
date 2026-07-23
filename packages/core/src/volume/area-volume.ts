/**
 * L3 area-volume composition — turns a positioned Area skeleton into an SDF field
 * + resolved sockets + content anchors. Per Space: pick a hull archetype, size it
 * to the envelope, displace with noise. Resolve every provisional socket onto its
 * Space's hull surface. Spline-tube the intra connectors. Compose the field
 * (smooth-union). Scatter anchors, binding the region's placed Locations + reserved
 * recipes. Nothing is meshed here — that's L4.
 */

import { Rng, fnv1a, boxSize, boxCenter, scale as vscale, type Vec3 } from "../math/index.js";
import type { PuzzleInstance } from "../puzzle/puzzle-def.js";
import type { AreaSkeleton, ProvisionalSocket, ResolvedSocket, SpaceSpec } from "../skeleton/space-plan.js";
import type { Sdf } from "./sdf.js";
import { hull } from "./hulls.js";
import { connectorTube } from "./spline.js";
import { composeAreaField, fieldExtentsFrom, type AreaField } from "./field.js";
import { resolveSocketPose } from "./socket-resolve.js";
import { scatterSpace, type ContentAnchor, type RequiredAnchor } from "../anchors/index.js";

const SOCKET_RADIUS = 2;

export interface AreaVolume {
  field: AreaField;
  resolvedSockets: ResolvedSocket[];
  anchors: ContentAnchor[];
}

export interface ComposeAreaVolumeParams {
  area: AreaSkeleton;
  regionLocations: { id: string; itemId?: string; sphere?: number }[];
  puzzleInstances: PuzzleInstance[];
  fidelityAngleStep: number | null;
  res?: number;
  margin?: number;
  maxDim?: number;
  rng: Rng;
}

function archetypeFor(s: SpaceSpec): string {
  if (s.outdoor) return "outdoor-open";
  if (s.role === "hub") return "rotunda";
  if (s.role === "capstone") return "cavern";
  if (s.role === "junction") return "shaft";
  return "hall";
}

function spaceHull(s: SpaceSpec): Sdf {
  const size = vscale(boxSize(s.envelope), 0.9);
  return hull(archetypeFor(s), { center: boxCenter(s.envelope), size, seed: fnv1a(s.id), noise: s.outdoor ? 1 : 0.4 });
}

export function composeAreaVolume(p: ComposeAreaVolumeParams): AreaVolume {
  const res = p.res ?? 1;
  const margin = p.margin ?? 3;
  const maxDim = p.maxDim ?? 72;
  const { area } = p;

  // 1. per-space hulls
  const hullBySpace = new Map<string, Sdf>();
  for (const s of area.spaces) hullBySpace.set(s.id, spaceHull(s));

  // 2. resolve sockets against their owning Space's hull
  const provBySpace = new Map<string, ProvisionalSocket>();
  const resolvedSockets: ResolvedSocket[] = [];
  const resolvedById = new Map<string, ResolvedSocket>();
  for (const prov of area.sockets) {
    provBySpace.set(prov.id, prov);
    const h = hullBySpace.get(prov.spaceId);
    if (!h) continue;
    const pose = resolveSocketPose(prov.pos, prov.dir, h, p.fidelityAngleStep, SOCKET_RADIUS);
    const rs: ResolvedSocket = {
      id: prov.id,
      spaceId: prov.spaceId,
      pos: pose.pos,
      basis: pose.basis,
      radius: SOCKET_RADIUS,
      kind: "structural",
      traversal: prov.traversal,
      signature: prov.signature,
      passable: pose.passable,
    };
    if (prov.gate !== undefined) rs.gate = prov.gate;
    if (prov.oneWay !== undefined) rs.oneWay = prov.oneWay;
    if (prov.partner !== undefined) rs.partner = prov.partner;
    resolvedSockets.push(rs);
    resolvedById.set(prov.id, rs);
  }

  // 3. intra connectors → spline tubes between resolved socket positions
  const tubes: Sdf[] = [];
  for (const c of area.connectors) {
    const from = resolvedById.get(c.from.socketId);
    const to = resolvedById.get(c.to.socketId);
    const fp = provBySpace.get(c.from.socketId);
    const tp = provBySpace.get(c.to.socketId);
    if (!from || !to || !fp || !tp) continue;
    tubes.push(connectorTube(from.pos, fp.dir, to.pos, tp.dir, SOCKET_RADIUS * 0.9));
  }

  // 4. compose the field
  const extents = fieldExtentsFrom(area.spaces.map((s) => s.envelope), res, margin, maxDim);
  const field = composeAreaField([...hullBySpace.values()], tubes, extents);

  // 5. anchors per Space (required manifest first)
  const anchors: ContentAnchor[] = [];
  const socketsBySpace = new Map<string, Vec3[]>();
  for (const rs of resolvedSockets) {
    const list = socketsBySpace.get(rs.spaceId) ?? [];
    list.push(rs.pos);
    socketsBySpace.set(rs.spaceId, list);
  }
  // distribute this region's placed Locations across the Spaces (skip entry Space first)
  const spaceOrder = [...area.spaces].sort((a, b) => (a.id === area.entrySpaceId ? 1 : b.id === area.entrySpaceId ? -1 : 0));
  const locBySpace = new Map<string, { id: string; itemId?: string; sphere?: number }[]>();
  p.regionLocations.forEach((loc, i) => {
    const s = spaceOrder[i % spaceOrder.length];
    if (!s) return;
    const list = locBySpace.get(s.id) ?? [];
    list.push(loc);
    locBySpace.set(s.id, list);
  });

  for (const s of area.spaces) {
    const h = field; // scatter against the composed field
    const required: RequiredAnchor[] = [];
    for (const loc of locBySpace.get(s.id) ?? []) {
      if (loc.itemId === undefined) continue; // only placed Locations get a pickup anchor
      const binding: RequiredAnchor["binding"] = { type: "location", locationId: loc.id, itemId: loc.itemId, ...(loc.sphere !== undefined ? { sphere: loc.sphere } : {}) };
      // best-effort: an undersized Space shouldn't crash generation (proper sizing = recipe envelopes / preset budgets)
      required.push({ kindId: "gadget-pickup", binding, optional: true });
    }
    for (const recipe of s.reservedRecipes) required.push({ kindId: "interactable", tags: [recipe] });
    if (s.landmark) required.push({ kindId: "landmark-feature", binding: { type: "landmark" }, optional: true });

    const spaceAnchors = scatterSpace({
      field: h.sdf,
      envelope: s.envelope,
      res,
      spaceId: s.id,
      socketPositions: socketsBySpace.get(s.id) ?? [],
      required,
      rng: p.rng.fork(`anchors:${s.id}`),
    });
    anchors.push(...spaceAnchors);
  }

  return { field, resolvedSockets, anchors };
}

// --- per-reach driver ---

import type { MissionGraph, Placement } from "../graph/index.js";
import type { ReachSkeleton } from "../skeleton/space-plan.js";

export interface ComposeReachVolumeOptions {
  seed: string;
  fidelityAngleStep: number | null;
  spheres?: string[][];
  res?: number;
  margin?: number;
  maxDim?: number;
}

/** Fill every Area's field/resolvedSockets/anchors from the mission graph + placement. */
export function composeReachVolume(
  skeleton: ReachSkeleton,
  graph: MissionGraph,
  placement: Placement,
  opts: ComposeReachVolumeOptions,
): void {
  const sphereOf = new Map<string, number>();
  (opts.spheres ?? []).forEach((sphere, idx) => {
    for (const loc of sphere) sphereOf.set(loc, idx);
  });

  for (const area of skeleton.areas) {
    const regionLocations = graph.locations
      .filter((l) => l.region === area.regionId)
      .map((l) => {
        const out: { id: string; itemId?: string; sphere?: number } = { id: l.id };
        const itemId = placement.get(l.id);
        if (itemId !== undefined) out.itemId = itemId;
        const sphere = sphereOf.get(l.id);
        if (sphere !== undefined) out.sphere = sphere;
        return out;
      });
    const vol = composeAreaVolume({
      area,
      regionLocations,
      puzzleInstances: [],
      fidelityAngleStep: opts.fidelityAngleStep,
      ...(opts.res !== undefined ? { res: opts.res } : {}),
      ...(opts.margin !== undefined ? { margin: opts.margin } : {}),
      ...(opts.maxDim !== undefined ? { maxDim: opts.maxDim } : {}),
      rng: new Rng(`${opts.seed}:vol:${area.regionId}`),
    });
    area.resolvedSockets = vol.resolvedSockets;
    area.anchors = vol.anchors;
    area.field = vol.field;
  }
}
