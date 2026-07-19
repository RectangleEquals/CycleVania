/**
 * RoomComposer — lays out a room's cells and contents WITHIN the extent envelope
 * the AreaComposer hands it. It does not choose the room's size (the AreaComposer
 * does); it decides HOW to fill the envelope:
 *   - a FOOTPRINT MASK so the floor plan is organic (hall / round / cavern) with
 *     air cells shaping the outer border — not a plain rectangle;
 *   - a guaranteed interior VOID (walkable air between floor and ceiling);
 *   - per-cell kit assignment (GeometryKit grammar + snap policy);
 *   - sockets carved-connected to the room core at required connectivity anchors;
 *   - content on interior floor cells only (never inside ceilings).
 */

import { Rng } from "../math/rng.js";
import { yawFromDirection } from "../math/trig.js";
import { add, scale, type Vec3, type WorldBox } from "../math/vec.js";
import { cellCenter, coordKey, type Coord } from "../spatial/grid.js";
import { faceNormal, type Socket } from "../spatial/sockets.js";
import { snapAngle } from "../spatial/snap.js";
import { piecesForRole, type KitPiece } from "../registries/geometry-kit.js";
import type { Registry } from "../registries/registry.js";
import type { CellFace, RoomKind, SocketKind, Traversal } from "../types.js";
import type { Rule } from "../logic/index.js";
import type { CellDescriptor, ContentAnchor, RoomDescriptor } from "../descriptors/descriptor.js";

export type RoomShape = "hall" | "round" | "cavern";

export interface SocketRequest {
  id: string;
  face: CellFace;
  kind?: SocketKind;
  traversal?: Traversal;
  gate?: Rule;
  oneWay?: boolean;
}

export interface ContentRequest {
  kind: string;
  ref?: string;
}

export interface RoomComposeParams {
  registry: Registry;
  seed: string | number;
  nodeId: string;
  regionId: string;
  role: string;
  kind: RoomKind;
  shape?: RoomShape;
  origin: Vec3;
  /** Extent in fine cells (the AreaComposer's envelope). */
  envelope: Coord;
  requiredSockets: SocketRequest[];
  contents: ContentRequest[];
  styleId: string;
  /** Mark this room a wave-lockdown arena. */
  arena?: { waves: number };
}

/** Representative boundary cell hosting the k-th of n sockets on a face. */
function faceCellSlot(face: CellFace, e: Coord, k: number, n: number): Coord {
  const mz = Math.min(1, e[2] - 1);
  const clampIn = (v: number, len: number): number => Math.max(0, Math.min(len - 1, v));
  const spread = (len: number): number => clampIn(Math.round(Math.floor(len / 2) + (k - (n - 1) / 2)), len);
  switch (face) {
    case "px":
      return [e[0] - 1, spread(e[1]), mz];
    case "nx":
      return [0, spread(e[1]), mz];
    case "py":
      return [spread(e[0]), e[1] - 1, mz];
    case "ny":
      return [spread(e[0]), 0, mz];
    case "pz":
      return [spread(e[0]), spread(e[1]), e[2] - 1];
    case "nz":
      return [spread(e[0]), spread(e[1]), 0];
    case "interior":
      return [Math.floor(e[0] / 2), Math.floor(e[1] / 2), mz];
  }
}

/** Build a per-column "inside" mask giving the room its footprint silhouette. */
function buildMask(shape: RoomShape, ex: number, ey: number, rng: Rng): boolean[][] {
  const cx = (ex - 1) / 2;
  const cy = (ey - 1) / 2;
  const rx = Math.max(1, ex / 2);
  const ry = Math.max(1, ey / 2);
  const mask: boolean[][] = Array.from({ length: ex }, () => new Array<boolean>(ey).fill(false));
  for (let x = 0; x < ex; x++) {
    for (let y = 0; y < ey; y++) {
      const nx = (x - cx) / rx;
      const ny = (y - cy) / ry;
      const d = Math.sqrt(nx * nx + ny * ny); // 0 centre … ~1 edge
      let inside: boolean;
      if (shape === "hall") inside = true;
      else if (shape === "round") inside = d <= 1.02;
      else inside = d <= 0.75 + 0.3 * rng.next(); // cavern: noisy radius
      // ragged erosion on the outer band (breaks the box; carves alcoves)
      if (inside && d > 0.6 && rng.chance(shape === "cavern" ? 0.3 : shape === "hall" ? 0.1 : 0.15)) inside = false;
      if (d < 0.5) inside = true; // protect a connected core
      (mask[x] as boolean[])[y] = inside;
    }
  }
  return mask;
}

/** Carve a straight run of "inside" cells (connects an opening to the core). */
function carveLine(mask: boolean[][], x0: number, y0: number, x1: number, y1: number): void {
  const steps = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0));
  for (let i = 0; i <= steps; i++) {
    const t = steps === 0 ? 0 : i / steps;
    const x = Math.round(x0 + (x1 - x0) * t);
    const y = Math.round(y0 + (y1 - y0) * t);
    const row = mask[x];
    if (row && y >= 0 && y < row.length) row[y] = true;
  }
}

export function composeRoom(p: RoomComposeParams): RoomDescriptor {
  const rng = new Rng(p.seed);
  const cs = p.registry.grid.roomCellSize;
  const snap = p.registry.grid.snap;
  const kit = p.registry.geometryKit;
  const shape: RoomShape = p.shape ?? "hall";

  // Extent: honour the envelope but guarantee an interior + a walkable void.
  const ex = Math.max(4, p.envelope[0]);
  const ey = Math.max(4, p.envelope[1]);
  const ez = Math.max(3, p.envelope[2]);
  const extent: Coord = [ex, ey, ez];

  const mask = buildMask(shape, ex, ey, rng);
  const cx = Math.floor(ex / 2);
  const cy = Math.floor(ey / 2);

  // Sockets → boundary cells, spread per face; carve each to the core so it connects.
  const sockets: Socket[] = [];
  const openingAt = new Map<string, Socket>();
  const byFace = new Map<CellFace, SocketRequest[]>();
  for (const req of p.requiredSockets) {
    const list = byFace.get(req.face) ?? [];
    list.push(req);
    byFace.set(req.face, list);
  }
  for (const [face, reqs] of byFace) {
    reqs.forEach((req, k) => {
      const cell = faceCellSlot(face, extent, k, reqs.length);
      carveLine(mask, cell[0], cell[1], cx, cy);
      const normal = faceNormal(face);
      const pos = add(cellCenter(cell, cs, p.origin), scale(normal, cs / 2));
      const socket: Socket = {
        id: req.id,
        cell,
        face,
        pos,
        dir: normal,
        width: cs,
        kind: req.kind ?? "arch",
        traversal: req.traversal ?? "walk",
        ...(req.gate ? { gate: req.gate } : {}),
        ...(req.oneWay ? { oneWay: true } : {}),
      };
      sockets.push(socket);
      openingAt.set(coordKey(cell), socket);
    });
  }

  const inside = (x: number, y: number): boolean => x >= 0 && x < ex && y >= 0 && y < ey && (mask[x] as boolean[])[y] === true;
  const isBorder = (x: number, y: number): boolean => !inside(x - 1, y) || !inside(x + 1, y) || !inside(x, y - 1) || !inside(x, y + 1);
  const isCorner = (x: number, y: number): boolean =>
    (!inside(x - 1, y) || !inside(x + 1, y)) && (!inside(x, y - 1) || !inside(x, y + 1));

  // --- interior features so rooms of different kinds read distinctly ---
  const pillar = new Set<string>(); // columns that are solid pillars
  const dais = new Set<string>(); // columns that carry a raised platform at z=1
  if ((p.shape === "hall" || p.kind === "hub" || p.kind === "junction") && ex >= 6 && ey >= 6) {
    for (let y = 2; y < ey - 2; y += 3) for (let x = 2; x < ex - 2; x += 3) if (inside(x, y) && !isBorder(x, y)) pillar.add(`${x},${y}`);
  }
  if ((p.kind === "vault" || p.kind === "boss-chamber" || p.kind === "shrine" || p.shape === "round") && ez >= 4) {
    const r = Math.min(ex, ey) / 4;
    for (let y = 0; y < ey; y++) for (let x = 0; x < ex; x++) if (inside(x, y) && !isBorder(x, y) && Math.hypot(x - cx, y - cy) <= r) dais.add(`${x},${y}`);
  }
  // multi-level: a mezzanine (raised partial floor) over one half of a tall room
  const mezz = new Set<string>();
  const mezZ = Math.floor(ez / 2);
  if (ez >= 5 && ex >= 7 && (p.kind === "hub" || p.kind === "boss-chamber" || p.kind === "indoor")) {
    const half = Math.floor(ex / 2);
    for (let y = 1; y < ey - 1; y++) for (let x = 1; x < half; x++) if (inside(x, y) && !isBorder(x, y)) mezz.add(`${x},${y}`);
  }

  // Interior floor cells for content (never a border, never a pillar/dais base).
  // Mezzanine columns ALSO get an upper anchor so the second level is worth the climb.
  const contentCells: Coord[] = [];
  for (let y = 0; y < ey; y++)
    for (let x = 0; x < ex; x++) {
      const c = `${x},${y}`;
      if (!inside(x, y) || isBorder(x, y) || pillar.has(c)) continue;
      contentCells.push([x, y, dais.has(c) ? 1 : 0]);
      if (mezz.has(c)) contentCells.push([x, y, mezZ]);
    }
  if (contentCells.length === 0) contentCells.push([cx, cy, 0]);

  const contentByCell = new Map<string, ContentAnchor[]>();
  p.contents.forEach((req, i) => {
    const cell = contentCells[i % contentCells.length] as Coord;
    const anchor: ContentAnchor = { kind: req.kind, pos: cellCenter(cell, cs, p.origin), ...(req.ref ? { ref: req.ref } : {}) };
    const list = contentByCell.get(coordKey(cell)) ?? [];
    list.push(anchor);
    contentByCell.set(coordKey(cell), list);
  });

  // adjacency-aware per-cell kit selection: filter candidates by the kit's
  // `forbidBeside` grammar against already-placed neighbours (greedy; a light WFC).
  const placedKit = new Map<string, KitPiece>();
  const pickPiece = (role: string, x: number, y: number, z: number): string | null => {
    const cands = piecesForRole(kit, role);
    if (cands.length === 0) return null;
    const neigh = [placedKit.get(`${x - 1},${y},${z}`), placedKit.get(`${x},${y - 1},${z}`), placedKit.get(`${x},${y},${z - 1}`)];
    const ok = cands.filter((pc) =>
      neigh.every((n) => !n || (!(pc.adjacency?.forbidBeside ?? []).includes(n.role) && !(n.adjacency?.forbidBeside ?? []).includes(role))),
    );
    const pool = ok.length > 0 ? ok : cands;
    const chosen = pool[Math.floor(rng.next() * pool.length)] as KitPiece;
    placedKit.set(`${x},${y},${z}`, chosen);
    return chosen.id;
  };

  const cells: CellDescriptor[] = [];
  for (let z = 0; z < ez; z++) {
    for (let y = 0; y < ey; y++) {
      for (let x = 0; x < ex; x++) {
        const coord: Coord = [x, y, z];
        const key = coordKey(coord);
        const socket = openingAt.get(key);
        const col = `${x},${y}`;
        let role: string;
        if (socket) role = "opening";
        else if (!inside(x, y)) role = "air";
        else if (z === 0) role = "floor";
        else if (z === ez - 1) role = "ceiling";
        else if (isBorder(x, y)) role = isCorner(x, y) ? "corner" : "wall";
        else if (dais.has(col) && z === 1) role = "floor"; // raised platform
        else if (mezz.has(col) && z === mezZ) role = "floor"; // mezzanine (upper level)
        else if (pillar.has(col) && z <= ez - 2) role = "corner"; // solid pillar column
        else role = "air"; // interior void — the walkable space
        const kitId = role === "air" ? null : pickPiece(role, x, y, z);
        const firstFace = socket?.face ?? (isBorder(x, y) && role !== "air" ? borderFace(inside, x, y) : undefined);
        const yaw = firstFace ? snapAngle(yawFromDirection(faceNormal(firstFace)[0], faceNormal(firstFace)[1]), snap) : 0;
        cells.push({
          coord,
          role,
          kitId,
          yaw,
          sockets: socket ? [socket] : [],
          contents: contentByCell.get(key) ?? [],
        });
      }
    }
  }

  const bounds: WorldBox = {
    min: p.origin,
    max: [p.origin[0] + ex * cs, p.origin[1] + ey * cs, p.origin[2] + ez * cs],
  };

  return {
    nodeId: p.nodeId,
    regionId: p.regionId,
    role: p.role,
    kind: p.kind,
    origin: p.origin,
    footprint: extent,
    cells,
    sockets,
    bounds,
    styleId: p.styleId,
    seed: String(p.seed),
    ...(p.arena ? { arena: p.arena } : {}),
  };
}

/** Compose a wave-lockdown arena room (a RoomComposer variant). */
export function composeArena(p: Omit<RoomComposeParams, "arena"> & { waves: number }): RoomDescriptor {
  return composeRoom({ ...p, arena: { waves: p.waves } });
}

/** Outward face of a border wall cell (points toward the nearest open side). */
function borderFace(inside: (x: number, y: number) => boolean, x: number, y: number): CellFace {
  if (!inside(x + 1, y)) return "px";
  if (!inside(x - 1, y)) return "nx";
  if (!inside(x, y + 1)) return "py";
  return "ny";
}
