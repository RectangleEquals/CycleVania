/**
 * AreaComposer — the "meat and potatoes". From ReachComposer state it decides an
 * area's room count, per-room extent envelopes, room kinds, and where portals to
 * neighbouring areas attach, then delegates each room's interior to the
 * RoomComposer. (The Phase A slice lays rooms in a row joined by straight
 * connectors; the full cyclic room-graph + arbitrary connector geometry lands in
 * Phase B — the descriptor shape is already Z/loop-ready.)
 */

import { yawFromDirection } from "../math/trig.js";
import { add, scale, type Vec3 } from "../math/vec.js";
import { boxUnionAll } from "../math/geom.js";
import { ruleCaps, type Capability, type Rule } from "../logic/index.js";
import { complexityFor } from "../registries/complexity-config.js";
import type { Registry } from "../registries/registry.js";
import type { CellFace, RoomKind } from "../types.js";
import type { RegionRole } from "../template/role.js";
import type { AreaDescriptor, ConnectorPlan, GadgetPlacement, PortalSpec, RoomDescriptor } from "../descriptors/descriptor.js";
import { composeRoom, type ContentRequest, type SocketRequest } from "./room-composer.js";

export interface PortalRequest {
  key: string;
  kind: "entry" | "exit";
  edge?: { from: string; to: string };
  requires: Rule;
  oneWay?: boolean;
}

export interface AreaComposeParams {
  registry: Registry;
  seed: string | number;
  areaId: number;
  regionId: string;
  role: RegionRole;
  depth: number;
  styleId: string;
  /** World offset (lane) so areas in a Reach don't overlap. */
  origin?: Vec3;
  portals: PortalRequest[];
  /** Location ids physically in this region. */
  locations: string[];
  /** locationId → itemId placed here (subset of the reach placement). */
  placement: Map<string, string>;
  /** itemId → capability granted (undefined for non-progression). */
  itemCap: Map<string, Capability | undefined>;
}

function kindOf(role: RegionRole): RoomKind {
  switch (role) {
    case "hub":
      return "hub";
    case "capstone":
      return "boss-chamber";
    case "vault":
      return "vault";
    case "terminal":
    case "gate":
      return "junction";
    default:
      return "indoor";
  }
}

const gateFields = (r: Rule, oneWay?: boolean): Partial<SocketRequest> => ({
  ...(r.k !== "always" ? { gate: r } : {}),
  ...(oneWay ? { oneWay: true } : {}),
});

export function composeArea(p: AreaComposeParams): AreaDescriptor {
  const budget = complexityFor(p.depth, p.registry.complexity);
  const cs = p.registry.grid.roomCellSize;

  let roomCount = Math.max(1, Math.min(4, 1 + Math.round(budget.c * 3)));
  if (p.role === "vault" || p.role === "terminal" || p.role === "capstone") roomCount = 1;

  const exy = Math.max(3, Math.min(6, 3 + Math.round(budget.c * 3)));
  const ez = Math.max(2, Math.min(3, 2 + Math.round(budget.zSpread * 2)));
  const lastIdx = roomCount - 1;

  const locByRoom: string[][] = Array.from({ length: roomCount }, () => []);
  p.locations.forEach((loc, i) => (locByRoom[i % roomCount] as string[]).push(loc));

  const entryPortals = p.portals.filter((x) => x.kind === "entry");
  const exitPortals = p.portals.filter((x) => x.kind === "exit");
  const exitFaces: CellFace[] = ["px", "py", "ny", "pz"];

  const origin: Vec3 = p.origin ?? [0, 0, 0];
  const rooms: RoomDescriptor[] = [];
  let cursorX = origin[0];
  for (let i = 0; i < roomCount; i++) {
    const sockReq: SocketRequest[] = [];
    if (i > 0) sockReq.push({ id: "in", face: "nx" });
    if (i < lastIdx) sockReq.push({ id: "out", face: "px" });
    if (i === 0) {
      for (const pr of entryPortals) sockReq.push({ id: `portal:${pr.key}`, face: "nx", ...gateFields(pr.requires, pr.oneWay) });
    }
    if (i === lastIdx) {
      exitPortals.forEach((pr, k) => sockReq.push({ id: `portal:${pr.key}`, face: exitFaces[k % exitFaces.length] as CellFace, ...gateFields(pr.requires, pr.oneWay) }));
    }

    const contents: ContentRequest[] = (locByRoom[i] as string[]).map((loc) => ({
      kind: p.placement.has(loc) ? "gadget" : "cache",
      ref: loc,
    }));

    const room = composeRoom({
      registry: p.registry,
      seed: `${p.seed}:area${p.areaId}:room${i}`,
      nodeId: `${p.regionId}#${i}`,
      regionId: p.regionId,
      role: p.role,
      kind: kindOf(p.role),
      origin: [cursorX, origin[1], origin[2]],
      envelope: [exy, exy, ez],
      requiredSockets: sockReq,
      contents,
      styleId: p.styleId,
    });
    rooms.push(room);
    cursorX = room.bounds.max[0] + cs * 1.5; // gap for a connector
  }

  // straight connectors between consecutive rooms
  const connectors: ConnectorPlan[] = [];
  for (let i = 0; i < lastIdx; i++) {
    const a = rooms[i] as RoomDescriptor;
    const b = rooms[i + 1] as RoomDescriptor;
    const outS = a.sockets.find((s) => s.id === "out");
    const inS = b.sockets.find((s) => s.id === "in");
    if (outS && inS) {
      connectors.push({ id: `c${i}`, fromRoom: a.nodeId, toRoom: b.nodeId, fromSocket: outS.id, toSocket: inS.id, kind: "straight", cells: [] });
    }
  }

  // portal specs from the realized portal sockets
  const portals: PortalSpec[] = [];
  for (const pr of p.portals) {
    const room = pr.kind === "entry" ? (rooms[0] as RoomDescriptor) : (rooms[lastIdx] as RoomDescriptor);
    const sock = room.sockets.find((s) => s.id === `portal:${pr.key}`);
    if (!sock) continue;
    const inward = scale(sock.dir, -1);
    const half = cs / 2;
    portals.push({
      key: pr.key,
      ...(pr.edge ? { edge: pr.edge } : {}),
      trigger: {
        min: [sock.pos[0] - half, sock.pos[1] - half, sock.pos[2] - half],
        max: [sock.pos[0] + half, sock.pos[1] + half, sock.pos[2] + half],
      },
      spawn: add(sock.pos, scale(inward, cs)),
      spawnYaw: yawFromDirection(inward[0], inward[1]),
      ...(pr.requires.k !== "always" ? { requires: pr.requires, requiredCaps: [...ruleCaps(pr.requires)] } : {}),
      ...(pr.oneWay ? { oneWay: true } : {}),
    });
  }

  // gadget placements derived from placed content anchors
  const gadgets: GadgetPlacement[] = [];
  for (const room of rooms) {
    for (const cell of room.cells) {
      for (const c of cell.contents) {
        if (c.kind === "gadget" && c.ref) {
          const itemId = p.placement.get(c.ref);
          if (itemId) {
            const cap = p.itemCap.get(itemId);
            gadgets.push({ itemId, ...(cap ? { cap } : {}), locationId: c.ref, pos: c.pos });
          }
        }
      }
    }
  }

  return {
    areaId: p.areaId,
    regionId: p.regionId,
    role: p.role,
    nodeGrid: { res: p.registry.grid.areaCellSize, dims: [roomCount, 1, 1] },
    rooms,
    connectors,
    portals,
    gadgets,
    bounds: boxUnionAll(rooms.map((r) => r.bounds)),
    styleId: p.styleId,
    seed: String(p.seed),
  };
}
