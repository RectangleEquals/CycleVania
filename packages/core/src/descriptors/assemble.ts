/**
 * Assembly — pure converters from the internal `ReachResult` / skeleton into the
 * serializable descriptor tree. Maps become sorted arrays; optional fields are set
 * only when present (never null); the SDF field function is dropped. Nothing here
 * decides anything — it converts.
 */

import { boxUnionAll, type WorldBox } from "../math/index.js";
import type { MissionGraph } from "../graph/index.js";
import type { AreaSkeleton, ConnectorSpec, ProvisionalSocket, ResolvedSocket } from "../skeleton/index.js";
import type { WorldComposer, ReachResult } from "../world/index.js";
import { GENERATION_VERSION } from "./version.js";
import type {
  AreaDescriptor,
  ConnectorDescriptor,
  MissionGraphData,
  ReachDescriptor,
  SocketData,
  SpaceDescriptor,
  Vec3Data,
  WorldBoxData,
  WorldDescriptor,
  WorldMetaData,
} from "./shapes.js";

const v3 = (v: readonly [number, number, number]): Vec3Data => [v[0], v[1], v[2]];
const boxData = (b: WorldBox): WorldBoxData => ({ min: v3(b.min), max: v3(b.max) });

function resolvedToSocketData(s: ResolvedSocket): SocketData {
  const out: SocketData = {
    id: s.id,
    spaceId: s.spaceId,
    pos: v3(s.pos),
    basis: { forward: v3(s.basis.forward), up: v3(s.basis.up), right: v3(s.basis.right) },
    radius: s.radius,
    kind: s.kind,
    traversal: s.traversal,
    signature: s.signature,
    passable: s.passable,
  };
  if (s.gate !== undefined) out.gate = s.gate;
  if (s.oneWay !== undefined) out.oneWay = s.oneWay;
  if (s.partner !== undefined) out.partner = s.partner;
  return out;
}

function provToSocketData(s: ProvisionalSocket): SocketData {
  const out: SocketData = { id: s.id, spaceId: s.spaceId, pos: v3(s.pos), kind: s.kind, traversal: s.traversal, signature: s.signature };
  if (s.gate !== undefined) out.gate = s.gate;
  if (s.oneWay !== undefined) out.oneWay = s.oneWay;
  if (s.partner !== undefined) out.partner = s.partner;
  return out;
}

function connectorData(c: ConnectorSpec): ConnectorDescriptor {
  const out: ConnectorDescriptor = { id: c.id, from: c.from, to: c.to, kind: c.kind, traversal: c.traversal };
  if (c.gate !== undefined) out.gate = c.gate;
  if (c.oneWay !== undefined) out.oneWay = c.oneWay;
  if (c.waypoints !== undefined) out.waypoints = c.waypoints.map(v3);
  return out;
}

function assembleArea(a: AreaSkeleton): AreaDescriptor {
  const bounds = a.spaces.length > 0 ? boxData(boxUnionAll(a.spaces.map((s) => s.envelope))) : { min: [0, 0, 0] as Vec3Data, max: [0, 0, 0] as Vec3Data };
  const socketsBySpace = new Map<string, SocketData[]>();
  const socketSource = a.resolvedSockets ?? a.sockets;
  for (const s of socketSource) {
    const data = "basis" in s ? resolvedToSocketData(s as ResolvedSocket) : provToSocketData(s as ProvisionalSocket);
    const list = socketsBySpace.get(data.spaceId) ?? [];
    list.push(data);
    socketsBySpace.set(data.spaceId, list);
  }
  const biome = a.spaces[0]?.biome ?? "default";
  const spaces: SpaceDescriptor[] = a.spaces.map((s) => {
    const sd: SpaceDescriptor = {
      id: s.id,
      kind: s.kind,
      role: s.role,
      regionId: s.regionId,
      origin: v3(s.origin),
      bounds: boxData(s.envelope),
      outdoor: s.outdoor,
      landmark: s.landmark,
      hidden: s.hidden,
      biome: s.biome,
      recipeIds: [...s.reservedRecipes],
      sockets: socketsBySpace.get(s.id) ?? [],
    };
    if (s.sub !== undefined) sd.subBiome = { id: s.sub.biome, blend: s.sub.blend };
    return sd;
  });

  const area: AreaDescriptor = {
    regionId: a.regionId,
    role: a.role,
    biome,
    bounds,
    spaces,
    connectors: a.connectors.map(connectorData),
    anchors: a.anchors ?? [],
  };
  if (a.finish) {
    area.kit = a.finish.kit;
    area.instances = a.finish.instances;
    area.occupancy = a.finish.occupancyData;
    area.dressing = a.finish.dressing;
  }
  return area;
}

function graphData(g: MissionGraph): MissionGraphData {
  return {
    regions: g.regions.map((r) => ({ id: r.id, role: r.role })),
    edges: g.edges.map((e) => (e.oneWay !== undefined ? { from: e.from, to: e.to, rule: e.rule, oneWay: e.oneWay } : { from: e.from, to: e.to, rule: e.rule })),
    flags: g.flags.map((f) => (f.volatile !== undefined ? { name: f.name, setBy: f.setBy, volatile: f.volatile } : { name: f.name, setBy: f.setBy })),
    locations: g.locations.map((l) => {
      const out: MissionGraphData["locations"][number] = { id: l.id, region: l.region };
      if (l.gate !== undefined) out.gate = l.gate;
      if (l.bonus !== undefined) out.bonus = l.bonus;
      return out;
    }),
    start: g.start,
  };
}

export function assembleReach(rr: ReachResult): ReachDescriptor {
  return {
    meta: {
      reachIndex: rr.meta.reachIndex,
      requestIdentity: rr.meta.requestIdentity,
      chosenModifiers: [...rr.meta.chosenModifiers],
      finalCeiling: rr.meta.finalCeiling,
      areaCount: rr.meta.areaCount,
      buckets: rr.buckets,
      spheres: rr.meta.spheres,
      relaxations: rr.meta.relaxations,
      startHeld: rr.meta.startHeld,
    },
    graph: graphData(rr.graph),
    placement: [...rr.placement.entries()].sort((a, b) => (a[0] < b[0] ? -1 : 1)).map(([locationId, itemId]) => ({ locationId, itemId })),
    puzzles: rr.puzzleInstances,
    areas: rr.skeleton.areas.map(assembleArea),
    interAreaConnectors: rr.skeleton.interAreaConnectors.map(connectorData),
  };
}

export function assembleWorld(w: WorldComposer): WorldDescriptor {
  const meta: WorldMetaData = {
    worldSeed: w.seed,
    generationVersion: GENERATION_VERSION,
    registryFingerprint: w.fingerprint,
    requestLog: w.requestLog,
  };
  if (w.lengthPolicy !== undefined) meta.lengthPolicy = w.lengthPolicy.max !== undefined ? { min: w.lengthPolicy.min, max: w.lengthPolicy.max } : { min: w.lengthPolicy.min };
  if (w.drawnLength !== undefined) meta.drawnLength = w.drawnLength;

  const reaches = [...w.realized.values()].sort((a, b) => a.meta.reachIndex - b.meta.reachIndex).map(assembleReach);
  return { meta, reachPortals: w.portals, reaches };
}
