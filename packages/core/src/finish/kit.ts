/**
 * Kit partitioner — split the dual-contoured mesh into modular, world-grid-aligned
 * pieces (one per grid cell carrying surface), in cell-local coords, deduplicated
 * by CANONICAL YAW: a piece and its 4 cardinal rotations collapse to one canonical
 * buffer, each instance recording the rotation to restore it. 5° fidelity makes
 * many cells identical → pieces ≪ instances. Budgets throw a loud GenError.
 */

import { fnv1a, type Vec3 } from "../math/index.js";
import { GenError } from "../errors.js";
import type { WorldBox } from "../math/index.js";
import type { SurfaceKind } from "../types.js";
import type { Coord } from "../volume/field.js";
import type { Mesh } from "./mesher.js";

export interface PieceMeta {
  surface: SurfaceKind | "mixed";
  biome: string;
  materialHint: string;
  collider: "solid" | "none" | "trigger";
  tags: string[];
}

export interface GeneratedPiece {
  id: string;
  positions: number[];
  normals: number[];
  indices: number[];
  meta: PieceMeta;
}

export interface PieceInstance {
  coord: Coord;
  pieceId: string;
  yaw: number;
}

export interface GeneratedKit {
  cellSize: number;
  pieces: GeneratedPiece[];
}

export interface KitOptions {
  biome: string;
  materialHint?: string;
  polyBudget?: number;
  maxUniquePieces?: number;
  revealableBoxes?: WorldBox[];
}

const round = (v: number): number => Math.round(v * 1000) / 1000;
const coordKey = (c: Coord): string => `${c[0]},${c[1]},${c[2]}`;

function classify(avgNz: number): SurfaceKind {
  if (avgNz > 0.6) return "floor";
  if (avgNz < -0.6) return "ceiling";
  if (avgNz > 0.25) return "slope";
  if (avgNz < -0.25) return "overhang";
  return "wall";
}

/** Rotate (x,y) by k×90° CCW about the cell center (s/2, s/2). */
function rotXY(x: number, y: number, s: number, k: number): [number, number] {
  let px = x - s / 2;
  let py = y - s / 2;
  for (let r = 0; r < k; r++) {
    const t = px;
    px = -py;
    py = t;
  }
  return [round(px + s / 2), round(py + s / 2)];
}
function rotNXY(nx: number, ny: number, k: number): [number, number] {
  let px = nx;
  let py = ny;
  for (let r = 0; r < k; r++) {
    const t = px;
    px = -py;
    py = t;
  }
  return [round(px), round(py)];
}

interface Local {
  positions: number[];
  normals: number[];
  indices: number[];
}

function rotatePiece(loc: Local, s: number, k: number): Local {
  if (k === 0) return loc;
  const positions: number[] = [];
  for (let v = 0; v < loc.positions.length; v += 3) {
    const [rx, ry] = rotXY(loc.positions[v] as number, loc.positions[v + 1] as number, s, k);
    positions.push(rx, ry, round(loc.positions[v + 2] as number));
  }
  const normals: number[] = [];
  for (let v = 0; v < loc.normals.length; v += 3) {
    const [rx, ry] = rotNXY(loc.normals[v] as number, loc.normals[v + 1] as number, k);
    normals.push(rx, ry, round(loc.normals[v + 2] as number));
  }
  return { positions, normals, indices: loc.indices.slice() };
}

const serialize = (l: Local): string => `${l.positions.join(",")}|${l.normals.join(",")}|${l.indices.join(",")}`;

const inBox = (b: WorldBox, p: Vec3): boolean =>
  p[0] >= b.min[0] && p[0] <= b.max[0] && p[1] >= b.min[1] && p[1] <= b.max[1] && p[2] >= b.min[2] && p[2] <= b.max[2];

export function meshToKit(mesh: Mesh, origin: Vec3, cellSize: number, opts: KitOptions): { kit: GeneratedKit; instances: PieceInstance[] } {
  const { positions: pos, normals: nrm, indices: idx } = mesh;
  const cells = new Map<string, { coord: Coord; tris: number[]; center: Vec3 }>();
  for (let t = 0; t < idx.length; t += 3) {
    const a = idx[t] as number;
    const b = idx[t + 1] as number;
    const c = idx[t + 2] as number;
    const cx = ((pos[a * 3] as number) + (pos[b * 3] as number) + (pos[c * 3] as number)) / 3;
    const cy = ((pos[a * 3 + 1] as number) + (pos[b * 3 + 1] as number) + (pos[c * 3 + 1] as number)) / 3;
    const cz = ((pos[a * 3 + 2] as number) + (pos[b * 3 + 2] as number) + (pos[c * 3 + 2] as number)) / 3;
    const coord: Coord = [Math.floor((cx - origin[0]) / cellSize), Math.floor((cy - origin[1]) / cellSize), Math.floor((cz - origin[2]) / cellSize)];
    const key = coordKey(coord);
    let e = cells.get(key);
    if (!e) {
      e = { coord, tris: [], center: [cx, cy, cz] };
      cells.set(key, e);
    }
    e.tris.push(a, b, c);
  }

  const pieces: GeneratedPiece[] = [];
  const byKey = new Map<string, string>();
  const instances: PieceInstance[] = [];

  for (const { coord, tris } of cells.values()) {
    const cox = origin[0] + coord[0] * cellSize;
    const coy = origin[1] + coord[1] * cellSize;
    const coz = origin[2] + coord[2] * cellSize;
    const remap = new Map<number, number>();
    const local: Local = { positions: [], normals: [], indices: [] };
    let sumNz = 0;
    let nCount = 0;
    for (const v of tris) {
      let li = remap.get(v);
      if (li === undefined) {
        li = local.positions.length / 3;
        remap.set(v, li);
        local.positions.push(round((pos[v * 3] as number) - cox), round((pos[v * 3 + 1] as number) - coy), round((pos[v * 3 + 2] as number) - coz));
        local.normals.push(round(nrm[v * 3] as number), round(nrm[v * 3 + 1] as number), round(nrm[v * 3 + 2] as number));
        sumNz += nrm[v * 3 + 2] as number;
        nCount++;
      }
      local.indices.push(li);
    }
    const cellCenter: Vec3 = [cox + cellSize / 2, coy + cellSize / 2, coz + cellSize / 2];
    const revealable = (opts.revealableBoxes ?? []).some((b) => inBox(b, cellCenter));

    // canonical yaw: min over the 4 rotations
    const rots = [0, 1, 2, 3].map((k) => rotatePiece(local, cellSize, k));
    const serials = rots.map(serialize);
    let m = 0;
    for (let k = 1; k < 4; k++) if ((serials[k] as string) < (serials[m] as string)) m = k;
    const canonical = rots[m] as Local;
    const key = (serials[m] as string) + (revealable ? "|rev" : "");

    let pieceId = byKey.get(key);
    if (pieceId === undefined) {
      pieceId = "gp" + pieces.length.toString(36);
      byKey.set(key, pieceId);
      pieces.push({
        id: pieceId,
        positions: canonical.positions,
        normals: canonical.normals,
        indices: canonical.indices,
        meta: {
          surface: classify(nCount > 0 ? sumNz / nCount : 0),
          biome: opts.biome,
          materialHint: opts.materialHint ?? opts.biome,
          collider: revealable ? "none" : "solid",
          tags: revealable ? ["revealable"] : [],
        },
      });
    }
    instances.push({ coord, pieceId, yaw: ((4 - m) % 4) * (Math.PI / 2) });
  }

  // budgets
  const pieceById = new Map(pieces.map((p) => [p.id, p] as const));
  let totalTris = 0;
  for (const inst of instances) totalTris += (pieceById.get(inst.pieceId)?.indices.length ?? 0) / 3;
  if (opts.maxUniquePieces !== undefined && pieces.length > opts.maxUniquePieces) {
    const worst = [...pieces].sort((a, b) => b.indices.length - a.indices.length).slice(0, 3).map((p) => ({ id: p.id, tris: p.indices.length / 3 }));
    throw new GenError("finish.piece-budget", `${pieces.length} unique pieces exceeds max ${opts.maxUniquePieces}`, { unique: pieces.length, max: opts.maxUniquePieces, worst });
  }
  if (opts.polyBudget !== undefined && totalTris > opts.polyBudget) {
    const worst = [...pieces].sort((a, b) => b.indices.length - a.indices.length).slice(0, 3).map((p) => ({ id: p.id, tris: p.indices.length / 3 }));
    throw new GenError("finish.poly-budget", `${totalTris} triangles exceeds budget ${opts.polyBudget}`, { tris: totalTris, budget: opts.polyBudget, worst });
  }

  return { kit: { cellSize, pieces }, instances };
}
