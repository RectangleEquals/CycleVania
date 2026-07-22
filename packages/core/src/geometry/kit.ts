/**
 * Kit partitioner — split the dual-contoured area mesh into modular, world-grid-aligned
 * pieces (one per grid cell that carries surface), each stored in cell-local coordinates.
 * Identical local geometry (flat floors/walls repeat a lot at 5° fidelity) is deduplicated
 * by content hash → a small GeneratedKit of unique pieces + a PieceInstance per filled cell.
 */

import type { Vec3 } from "../math/vec.js";
import type { Coord } from "../spatial/grid.js";
import { coordKey } from "../spatial/grid.js";
import { fnv1a } from "../math/rng.js";
import type { Mesh } from "./mesher.js";

export type SurfaceKind = "floor" | "wall" | "ceiling" | "slope" | "overhang" | "mixed";

export interface PieceMeta {
  surface: SurfaceKind;
  biome: string;
  materialHint: string;
  collider: "solid" | "none" | "trigger";
  tags: string[];
}

export interface GeneratedPiece {
  id: string;
  positions: number[]; // local to the cell origin
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
}

const round = (v: number, q = 1e3): number => Math.round(v * q) / q;

function classify(avgNz: number): SurfaceKind {
  if (avgNz > 0.6) return "floor"; // normal points up into the room
  if (avgNz < -0.6) return "ceiling";
  if (avgNz > 0.25) return "slope";
  if (avgNz < -0.25) return "overhang";
  return "wall";
}

export function meshToKit(mesh: Mesh, origin: Vec3, cellSize: number, opts: KitOptions): { kit: GeneratedKit; instances: PieceInstance[] } {
  const pos = mesh.positions;
  const nrm = mesh.normals;
  const idx = mesh.indices;
  const cells = new Map<string, { coord: Coord; tris: number[] }>();
  for (let t = 0; t < idx.length; t += 3) {
    const a = idx[t] as number;
    const b = idx[t + 1] as number;
    const c = idx[t + 2] as number;
    const cx = ((pos[a * 3] as number) + (pos[b * 3] as number) + (pos[c * 3] as number)) / 3;
    const cy = ((pos[a * 3 + 1] as number) + (pos[b * 3 + 1] as number) + (pos[c * 3 + 1] as number)) / 3;
    const cz = ((pos[a * 3 + 2] as number) + (pos[b * 3 + 2] as number) + (pos[c * 3 + 2] as number)) / 3;
    const coord: Coord = [
      Math.floor((cx - origin[0]) / cellSize),
      Math.floor((cy - origin[1]) / cellSize),
      Math.floor((cz - origin[2]) / cellSize),
    ];
    const key = coordKey(coord);
    let e = cells.get(key);
    if (!e) {
      e = { coord, tris: [] };
      cells.set(key, e);
    }
    e.tris.push(a, b, c);
  }

  const pieces: GeneratedPiece[] = [];
  const byHash = new Map<string, string>();
  const instances: PieceInstance[] = [];

  for (const { coord, tris } of cells.values()) {
    const cox = origin[0] + coord[0] * cellSize;
    const coy = origin[1] + coord[1] * cellSize;
    const coz = origin[2] + coord[2] * cellSize;
    const remap = new Map<number, number>();
    const positions: number[] = [];
    const normals: number[] = [];
    const indices: number[] = [];
    let sumNz = 0;
    let nCount = 0;
    for (const v of tris) {
      let li = remap.get(v);
      if (li === undefined) {
        li = positions.length / 3;
        remap.set(v, li);
        positions.push(round((pos[v * 3] as number) - cox), round((pos[v * 3 + 1] as number) - coy), round((pos[v * 3 + 2] as number) - coz));
        normals.push(round(nrm[v * 3] as number), round(nrm[v * 3 + 1] as number), round(nrm[v * 3 + 2] as number));
        sumNz += nrm[v * 3 + 2] as number;
        nCount++;
      }
      indices.push(li);
    }
    const h = fnv1a(positions.join(",") + "|" + normals.join(",") + "|" + indices.join(",")).toString(36);
    let pieceId = byHash.get(h);
    if (pieceId === undefined) {
      pieceId = "gp" + pieces.length.toString(36);
      byHash.set(h, pieceId);
      pieces.push({
        id: pieceId,
        positions,
        normals,
        indices,
        meta: {
          surface: classify(nCount > 0 ? sumNz / nCount : 0),
          biome: opts.biome,
          materialHint: opts.materialHint ?? opts.biome,
          collider: "solid",
          tags: [],
        },
      });
    }
    instances.push({ coord, pieceId, yaw: 0 });
  }

  return { kit: { cellSize, pieces }, instances };
}
