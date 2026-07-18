/**
 * RoomComposer — lays out a room's cells and contents WITHIN the extent envelope
 * the AreaComposer hands it. It does not choose the room's size (that's the
 * AreaComposer's call); it decides how to fill the envelope: per-cell kit
 * assignment (via the GeometryKit grammar + snap policy), socket realization at
 * required connectivity anchors, and content placement at requested anchors.
 */

import { Rng } from "../math/rng.js";
import { yawFromDirection } from "../math/trig.js";
import { add, scale, type Vec3, type WorldBox } from "../math/vec.js";
import { cellCenter, coordKey, type Coord } from "../spatial/grid.js";
import { classifyCell, type CellRole } from "../spatial/cell.js";
import { faceNormal, type Socket } from "../spatial/sockets.js";
import { snapAngle } from "../spatial/snap.js";
import { piecesForRole } from "../registries/geometry-kit.js";
import type { Registry } from "../registries/registry.js";
import type { CellFace, RoomKind, SocketKind, Traversal } from "../types.js";
import type { Rule } from "../logic/index.js";
import type { CellDescriptor, ContentAnchor, RoomDescriptor } from "../descriptors/descriptor.js";

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
  /** World min corner of the room. */
  origin: Vec3;
  /** Max extent in fine cells (the AreaComposer's envelope). */
  envelope: Coord;
  requiredSockets: SocketRequest[];
  contents: ContentRequest[];
  styleId: string;
}

/** Boundary cell hosting the `k`-th of `n` sockets on a face (spread along it). */
function faceCellSlot(face: CellFace, e: Coord, k: number, n: number): Coord {
  const mz = Math.min(1, e[2] - 1);
  const clampIn = (v: number, len: number): number => Math.max(1, Math.min(len - 2, v));
  // spread offset around centre for the k-th of n sockets
  const spread = (len: number): number => clampIn(Math.floor(len / 2) + (k - (n - 1) / 2) | 0, len);
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

export function composeRoom(p: RoomComposeParams): RoomDescriptor {
  const rng = new Rng(p.seed);
  const cs = p.registry.grid.roomCellSize;
  const snap = p.registry.grid.snap;
  const kit = p.registry.geometryKit;

  // Settle the used extent within the envelope (may shrink; never below 2).
  const shrink = (max: number, min = 2): number => Math.max(min, rng.int(Math.max(min, max - 1), Math.max(min, max)));
  const extent: Coord = [shrink(p.envelope[0]), shrink(p.envelope[1]), shrink(p.envelope[2], 2)];

  // Map required sockets to boundary cells, spreading multiples on the same face.
  const sockets: Socket[] = [];
  const openingAt = new Map<string, Socket>(); // coordKey → socket
  const byFace = new Map<CellFace, SocketRequest[]>();
  for (const req of p.requiredSockets) {
    const list = byFace.get(req.face) ?? [];
    list.push(req);
    byFace.set(req.face, list);
  }
  for (const [face, reqs] of byFace) {
    reqs.forEach((req, k) => {
      const cell = faceCellSlot(face, extent, k, reqs.length);
      const normal = faceNormal(face);
      const pos = add(cellCenter(cell, cs, p.origin), scale(normal, cs / 2)); // on the face plane
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

  // Candidate content cells: interior/floor cells near the centre.
  const contentCells: Coord[] = [];
  for (let y = 1; y < extent[1] - 1; y++) for (let x = 1; x < extent[0] - 1; x++) contentCells.push([x, y, 0]);
  if (contentCells.length === 0) contentCells.push([Math.floor(extent[0] / 2), Math.floor(extent[1] / 2), 0]);

  // Build every cell.
  const cells: CellDescriptor[] = [];
  const contentByCell = new Map<string, ContentAnchor[]>();
  p.contents.forEach((req, i) => {
    const cell = contentCells[i % contentCells.length] as Coord;
    const anchor: ContentAnchor = { kind: req.kind, pos: cellCenter(cell, cs, p.origin), ...(req.ref ? { ref: req.ref } : {}) };
    const list = contentByCell.get(coordKey(cell)) ?? [];
    list.push(anchor);
    contentByCell.set(coordKey(cell), list);
  });

  for (let z = 0; z < extent[2]; z++) {
    for (let y = 0; y < extent[1]; y++) {
      for (let x = 0; x < extent[0]; x++) {
        const coord: Coord = [x, y, z];
        const key = coordKey(coord);
        const socket = openingAt.get(key);
        const cls = classifyCell(coord, extent);
        const role: CellRole | "opening" = socket ? "opening" : cls.role;
        const pieces = role === "air" ? [] : piecesForRole(kit, role);
        const kitId = pieces.length > 0 ? rng.pick(pieces).id : null;
        const firstFace = socket?.face ?? cls.faces[0];
        const yaw = firstFace && firstFace !== "interior" ? snapAngle(yawFromDirection(faceNormal(firstFace)[0], faceNormal(firstFace)[1]), snap) : 0;
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
    max: [p.origin[0] + extent[0] * cs, p.origin[1] + extent[1] * cs, p.origin[2] + extent[2] * cs],
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
  };
}
