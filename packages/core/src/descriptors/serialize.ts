/**
 * Canonical serialization + host helpers. `stableStringify` emits object keys in
 * sorted order and arrays in place, so two equal worlds produce byte-equal JSON
 * (the determinism/diff contract). `toTypedKit` converts the `number[]` buffers to
 * typed arrays at load.
 */

import type { GeneratedKit } from "../finish/index.js";

export function stableStringify(x: unknown): string {
  if (x === null || x === undefined || typeof x !== "object") return JSON.stringify(x) ?? "null";
  if (Array.isArray(x)) return `[${x.map(stableStringify).join(",")}]`;
  const obj = x as Record<string, unknown>;
  const keys = Object.keys(obj).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stableStringify(obj[k])}`).join(",")}}`;
}

export interface TypedPiece {
  id: string;
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
  meta: unknown;
}
export interface TypedKit {
  cellSize: number;
  pieces: TypedPiece[];
}

export function toTypedKit(kit: GeneratedKit): TypedKit {
  return {
    cellSize: kit.cellSize,
    pieces: kit.pieces.map((p) => ({
      id: p.id,
      positions: Float32Array.from(p.positions),
      normals: Float32Array.from(p.normals),
      indices: Uint32Array.from(p.indices),
      meta: p.meta,
    })),
  };
}
