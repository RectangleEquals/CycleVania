/**
 * AreaComposer — the "meat and potatoes". From ReachComposer state it decides an
 * area's room count, per-room extent envelopes + shapes, and lays the room-nodes
 * out DIEGETICALLY on the coarse grid: a branching walk that fans across XY and
 * steps in ±Z (never a straight line), non-overlapping, joined by connectors that
 * carry real socket endpoints (walk or vertical). Portals to neighbouring areas
 * attach on free outward faces. Room interiors are delegated to the RoomComposer.
 */

import { Rng } from "../math/rng.js";
import { yawFromDirection } from "../math/trig.js";
import { add, scale, type Vec3, type WorldBox } from "../math/vec.js";
import { boxOverlap, boxUnionAll } from "../math/geom.js";
import { oppositeFace } from "../spatial/sockets.js";
import { ruleCaps, type Capability, type Rule } from "../logic/index.js";
import { complexityFor } from "../registries/complexity-config.js";
import type { Registry } from "../registries/registry.js";
import type { CellFace, ConnectorKind, RoomKind } from "../types.js";
import type { RegionRole } from "../template/role.js";
import type { AreaDescriptor, ConnectorPlan, GadgetPlacement, PortalSpec, RoomDescriptor } from "../descriptors/descriptor.js";
import { composeRoom, type ContentRequest, type RoomShape, type SocketRequest } from "./room-composer.js";
import { corridorGeometry } from "./connector-composer.js";
import type { Coord } from "../spatial/grid.js";

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
  origin?: Vec3;
  portals: PortalRequest[];
  locations: string[];
  placement: Map<string, string>;
  itemCap: Map<string, Capability | undefined>;
}

const ALL_FACES: CellFace[] = ["px", "nx", "py", "ny", "pz", "nz"];
const DIRS: Array<{ v: Vec3; face: CellFace }> = [
  { v: [1, 0, 0], face: "px" },
  { v: [-1, 0, 0], face: "nx" },
  { v: [0, 1, 0], face: "py" },
  { v: [0, -1, 0], face: "ny" },
  { v: [0, 0, 1], face: "pz" },
  { v: [0, 0, -1], face: "nz" },
];

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

function shapeOf(kind: RoomKind, rng: Rng): RoomShape {
  switch (kind) {
    case "hub":
    case "junction":
      return "hall";
    case "vault":
    case "shrine":
    case "boss-chamber":
      return "round";
    case "outdoor":
      return "cavern";
    default:
      return rng.chance(0.5) ? "cavern" : "hall";
  }
}

const gateFields = (r: Rule, oneWay?: boolean): Partial<SocketRequest> => ({
  ...(r.k !== "always" ? { gate: r } : {}),
  ...(oneWay ? { oneWay: true } : {}),
});

export function composeArea(p: AreaComposeParams): AreaDescriptor {
  const rng = new Rng(`${p.seed}:area${p.areaId}:layout`);
  const budget = complexityFor(p.depth, p.registry.complexity);
  const cs = p.registry.grid.roomCellSize;
  const origin: Vec3 = p.origin ?? [0, 0, 0];

  const kind = kindOf(p.role);
  let roomCount: number;
  if (p.role === "vault" || p.role === "terminal") roomCount = 1;
  else if (p.role === "capstone") roomCount = Math.max(1, Math.min(2, 1 + Math.round(budget.c)));
  else if (p.role === "hub") roomCount = Math.max(2, Math.min(5, 2 + Math.round(budget.c * 3)));
  else roomCount = Math.max(1, Math.min(6, 1 + Math.round(budget.c * 5)));

  // per-room extents (fine cells) + shapes
  const extents: Coord[] = [];
  const shapes: RoomShape[] = [];
  const big = p.role === "capstone" || p.role === "vault";
  for (let i = 0; i < roomCount; i++) {
    const exy = Math.max(4, Math.min(12, (big ? 8 : 5) + Math.round(budget.c * 5) + rng.int(-1, 1)));
    const ez = Math.max(3, Math.min(6, 3 + Math.round(budget.zSpread * 3) + rng.int(0, 1)));
    extents.push([exy, exy, ez]);
    shapes.push(shapeOf(kind, rng));
  }

  // --- place room-nodes: branching walk with Z, non-overlapping ---
  const gap = cs * 2.5;
  const origins: Vec3[] = [origin];
  const boxes: WorldBox[] = [boxAt(origin, extents[0] as Coord, cs)];
  const parentOf: number[] = [-1];
  const dirFaceOf: CellFace[] = ["interior"];
  const zBias = 0.15 + 0.55 * budget.zSpread;

  for (let i = 1; i < roomCount; i++) {
    const size = extents[i] as Coord;
    let placed = false;
    for (let attempt = 0; attempt < 24 && !placed; attempt++) {
      const j = attempt < 8 ? i - 1 : rng.int(0, i - 1); // prefer snaking, then branch
      const dir = pickDir(rng, zBias);
      const childOrigin = offsetOrigin(origins[j] as Vec3, extents[j] as Coord, size, dir.v, cs, gap);
      const box = boxAt(childOrigin, size, cs);
      if (!boxes.some((b) => boxOverlap(b, box, gap * 0.5))) {
        origins.push(childOrigin);
        boxes.push(box);
        parentOf.push(j);
        dirFaceOf.push(dir.face);
        placed = true;
      }
    }
    if (!placed) {
      // fallback: push far along +X of the current bounds
      const maxX = Math.max(...boxes.map((b) => b.max[0]));
      const childOrigin: Vec3 = [maxX + gap, origin[1], origin[2]];
      origins.push(childOrigin);
      boxes.push(boxAt(childOrigin, size, cs));
      parentOf.push(i - 1);
      dirFaceOf.push("px");
    }
  }

  // grow rooms into leftover space (avoid overlap) so the area isn't sparse
  for (let i = 0; i < roomCount; i++) {
    for (const axis of [0, 1] as const) {
      for (let step = 0; step < 3; step++) {
        const e = extents[i] as Coord;
        const grown: Coord = axis === 0 ? [e[0] + 1, e[1], e[2]] : [e[0], e[1] + 1, e[2]];
        if (grown[axis] > 14) break;
        const box = boxAt(origins[i] as Vec3, grown, cs);
        if (boxes.some((b, j) => j !== i && boxOverlap(b, box, gap * 0.4))) break;
        extents[i] = grown;
        boxes[i] = box;
      }
    }
  }

  // --- connectors + per-room socket requests ---
  const sockReq: SocketRequest[][] = Array.from({ length: roomCount }, () => []);
  const usedFaces: Set<CellFace>[] = Array.from({ length: roomCount }, () => new Set<CellFace>());
  const connectors: ConnectorPlan[] = [];
  const nodeId = (i: number): string => `${p.regionId}#${i}`;

  for (let i = 1; i < roomCount; i++) {
    const j = parentOf[i] as number;
    const parentFace = dirFaceOf[i] as CellFace;
    const childFace = oppositeFace(parentFace);
    sockReq[j]?.push({ id: `c${i}a`, face: parentFace });
    sockReq[i]?.push({ id: `c${i}b`, face: childFace });
    usedFaces[j]?.add(parentFace);
    usedFaces[i]?.add(childFace);
    const kindC: ConnectorKind = parentFace === "pz" || parentFace === "nz" ? "vertical" : "straight";
    connectors.push({ id: `c${i}`, fromRoom: nodeId(j), toRoom: nodeId(i), fromSocket: `c${i}a`, toSocket: `c${i}b`, kind: kindC, cells: [] });
  }

  // cyclic shortcuts: connect the NEAREST unconnected rooms → coherent local cycles
  if (roomCount >= 3) {
    const connected = new Set<string>();
    for (const c of connectors) connected.add([c.fromRoom, c.toRoom].sort().join("|"));
    const center = (b: WorldBox): Vec3 => [(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2];
    const pairs: Array<{ a: number; b: number; d: number }> = [];
    for (let a = 0; a < roomCount; a++)
      for (let b = a + 1; b < roomCount; b++) {
        const ca = center(boxes[a] as WorldBox);
        const cb = center(boxes[b] as WorldBox);
        pairs.push({ a, b, d: Math.hypot(ca[0] - cb[0], ca[1] - cb[1], ca[2] - cb[2]) });
      }
    pairs.sort((x, y) => x.d - y.d);
    const cap = 1 + Math.round(budget.extraCycles);
    let added = 0;
    for (const { a, b } of pairs) {
      if (added >= cap) break;
      const key = [nodeId(a), nodeId(b)].sort().join("|");
      if (connected.has(key)) continue;
      if (!rng.chance(0.35 + budget.loopChance * 0.5)) continue;
      const face = dominantFace(origins[a] as Vec3, origins[b] as Vec3);
      const opp = oppositeFace(face);
      if (usedFaces[a]?.has(face) || usedFaces[b]?.has(opp)) continue;
      const id = `cyc${added}`;
      sockReq[a]?.push({ id: `${id}A`, face });
      sockReq[b]?.push({ id: `${id}B`, face: opp });
      usedFaces[a]?.add(face);
      usedFaces[b]?.add(opp);
      connectors.push({ id, fromRoom: nodeId(a), toRoom: nodeId(b), fromSocket: `${id}A`, toSocket: `${id}B`, kind: "curved", cells: [] });
      connected.add(key);
      added++;
    }
  }

  // --- portals on free outward faces ---
  const freeFaces = (i: number): CellFace[] => ALL_FACES.filter((f) => !usedFaces[i]?.has(f));
  const entry = p.portals.filter((x) => x.kind === "entry");
  const exit = p.portals.filter((x) => x.kind === "exit");
  const portalTargets = new Map<string, number>(); // portal key → room index
  const assignPortal = (pr: PortalRequest, roomIdx: number): void => {
    const free = freeFaces(roomIdx);
    const face = free[0] ?? "pz";
    sockReq[roomIdx]?.push({ id: `portal:${pr.key}`, face, ...gateFields(pr.requires, pr.oneWay) });
    usedFaces[roomIdx]?.add(face);
    portalTargets.set(pr.key, roomIdx);
  };
  for (const pr of entry) assignPortal(pr, 0);
  // exits prefer leaves (rooms that are someone's parent least often), else round-robin
  let ri = roomCount - 1;
  for (const pr of exit) {
    let target = ri;
    for (let t = 0; t < roomCount; t++) {
      const cand = (ri - t + roomCount) % roomCount;
      if (freeFaces(cand).length > 0) {
        target = cand;
        break;
      }
    }
    assignPortal(pr, target);
    ri = (ri - 1 + roomCount) % roomCount;
  }

  // --- content distribution ---
  const locByRoom: string[][] = Array.from({ length: roomCount }, () => []);
  p.locations.forEach((loc, i) => (locByRoom[i % roomCount] as string[]).push(loc));

  // --- compose rooms ---
  const rooms: RoomDescriptor[] = [];
  for (let i = 0; i < roomCount; i++) {
    const contents: ContentRequest[] = (locByRoom[i] as string[]).map((loc) => ({ kind: p.placement.has(loc) ? "gadget" : "cache", ref: loc }));
    contents.push(...populate(p.role, extents[i] as Coord, budget.c, rng));
    rooms.push(
      composeRoom({
        registry: p.registry,
        seed: `${p.seed}:area${p.areaId}:room${i}`,
        nodeId: nodeId(i),
        regionId: p.regionId,
        role: p.role,
        kind,
        shape: shapes[i] as RoomShape,
        origin: origins[i] as Vec3,
        envelope: extents[i] as Coord,
        requiredSockets: sockReq[i] as SocketRequest[],
        contents,
        styleId: p.styleId,
        ...(p.role === "capstone" ? { arena: { waves: 1 + Math.round(budget.c * 2) } } : {}),
      }),
    );
  }

  // --- portal specs from realized sockets ---
  const portals: PortalSpec[] = [];
  for (const pr of p.portals) {
    const roomIdx = portalTargets.get(pr.key);
    if (roomIdx === undefined) continue;
    const room = rooms[roomIdx] as RoomDescriptor;
    const sock = room.sockets.find((s) => s.id === `portal:${pr.key}`);
    if (!sock) continue;
    const inward = scale(sock.dir, -1);
    const half = cs / 2;
    portals.push({
      key: pr.key,
      ...(pr.edge ? { edge: pr.edge } : {}),
      trigger: { min: [sock.pos[0] - half, sock.pos[1] - half, sock.pos[2] - half], max: [sock.pos[0] + half, sock.pos[1] + half, sock.pos[2] + half] },
      spawn: add(sock.pos, scale(inward, cs)),
      spawnYaw: yawFromDirection(inward[0], inward[1]),
      ...(pr.requires.k !== "always" ? { requires: pr.requires, requiredCaps: [...ruleCaps(pr.requires)] } : {}),
      ...(pr.oneWay ? { oneWay: true } : {}),
    });
  }

  // --- gadgets from placed content anchors ---
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

  // compose corridor geometry now that socket world-positions exist
  for (const c of connectors) {
    const fr = rooms.find((r) => r.nodeId === c.fromRoom);
    const to = rooms.find((r) => r.nodeId === c.toRoom);
    const fs = fr?.sockets.find((s) => s.id === c.fromSocket);
    const ts = to?.sockets.find((s) => s.id === c.toSocket);
    if (fs && ts) {
      const geom = corridorGeometry(fs.pos, ts.pos, p.registry, `${p.seed}:area${p.areaId}:${c.id}`);
      c.cells = geom.cells;
      c.origin = geom.origin;
      c.cellSize = geom.cellSize;
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

// --- helpers ---

function boxAt(origin: Vec3, extent: Coord, cs: number): WorldBox {
  return { min: origin, max: [origin[0] + extent[0] * cs, origin[1] + extent[1] * cs, origin[2] + extent[2] * cs] };
}

function pickDir(rng: Rng, zBias: number): { v: Vec3; face: CellFace } {
  if (rng.chance(zBias)) return DIRS[rng.chance(0.5) ? 4 : 5] as { v: Vec3; face: CellFace };
  return DIRS[rng.int(0, 3)] as { v: Vec3; face: CellFace };
}

function offsetOrigin(parentOrigin: Vec3, parentExt: Coord, childExt: Coord, dir: Vec3, cs: number, gap: number): Vec3 {
  const ps: Vec3 = [parentExt[0] * cs, parentExt[1] * cs, parentExt[2] * cs];
  const csz: Vec3 = [childExt[0] * cs, childExt[1] * cs, childExt[2] * cs];
  // centre-align on the non-travel axes so footprints line up
  const o: [number, number, number] = [
    parentOrigin[0] + (ps[0] - csz[0]) / 2,
    parentOrigin[1] + (ps[1] - csz[1]) / 2,
    parentOrigin[2] + (ps[2] - csz[2]) / 2,
  ];
  if (dir[0] > 0) o[0] = parentOrigin[0] + ps[0] + gap;
  else if (dir[0] < 0) o[0] = parentOrigin[0] - csz[0] - gap;
  else if (dir[1] > 0) o[1] = parentOrigin[1] + ps[1] + gap;
  else if (dir[1] < 0) o[1] = parentOrigin[1] - csz[1] - gap;
  else if (dir[2] > 0) o[2] = parentOrigin[2] + ps[2] + gap;
  else if (dir[2] < 0) o[2] = parentOrigin[2] - csz[2] - gap;
  return o;
}

/** Scatter non-progression content so rooms feel inhabited, not empty. */
function populate(role: RegionRole, extent: Coord, c: number, rng: Rng): ContentRequest[] {
  const out: ContentRequest[] = [];
  const floorArea = extent[0] * extent[1];
  const density = 0.45 + 0.55 * c;
  const props = Math.min(10, Math.round((floorArea / 16) * density));
  const pool: string[] =
    role === "capstone"
      ? ["enemy", "enemy", "hazard", "prop"]
      : role === "vault"
        ? ["prop", "prop", "hazard"]
        : role === "hub" || role === "terminal"
          ? ["prop", "prop"]
          : ["prop", "enemy", "hazard", "prop"];
  for (let i = 0; i < props; i++) out.push({ kind: rng.pick(pool) });
  if (role === "gate") out.push({ kind: "switch" });
  if (role === "capstone") out.push({ kind: "enemy", ref: "boss" });
  return out;
}

function dominantFace(a: Vec3, b: Vec3): CellFace {
  const d: Vec3 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  const ax = Math.abs(d[0]);
  const ay = Math.abs(d[1]);
  const az = Math.abs(d[2]);
  if (ax >= ay && ax >= az) return d[0] >= 0 ? "px" : "nx";
  if (ay >= az) return d[1] >= 0 ? "py" : "ny";
  return d[2] >= 0 ? "pz" : "nz";
}
