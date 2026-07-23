/**
 * Poisson-disk content-anchor scatter over the composed field's surface, within a
 * Space's envelope. Required manifest anchors (gadget pickups, recipe
 * interactables) place FIRST with a raised attempt budget — a failure is a loud
 * `GenError` (never a silent drop). Decorative kinds fill afterward. Enforces the
 * overlap invariants: cross-kind separation, structural clearance, on-surface.
 */

import { add, scale, sub, normalize, type Vec3, type WorldBox, type Rng } from "../math/index.js";
import { GenError } from "../errors.js";
import { sdfNormal, type Sdf } from "../volume/sdf.js";
import type { SurfaceKind } from "../types.js";
import { classifySurface } from "./surface-classify.js";
import { DEFAULT_ANCHOR_KINDS, type ContentAnchor, type ContentAnchorKind, type AnchorBinding } from "./anchor-kinds.js";

export interface RequiredAnchor {
  kindId: string;
  tags?: string[];
  binding?: AnchorBinding;
  /** Best-effort: skip (don't throw) if it cannot be placed (e.g., a landmark in a small Space). */
  optional?: boolean;
}

interface Candidate {
  pos: Vec3;
  up: Vec3;
  surface: SurfaceKind;
}

function projectToSurface(field: Sdf, p: Vec3): Vec3 {
  let q = p;
  for (let i = 0; i < 6; i++) {
    const d = field(q);
    if (Math.abs(d) < 1e-2) break;
    const n = sdfNormal(field, q);
    q = sub(q, scale(n, d));
  }
  return q;
}

function collectCandidates(field: Sdf, env: WorldBox, res: number): Candidate[] {
  const out: Candidate[] = [];
  // cap total samples (~2000) so scatter cost stays bounded on large envelopes
  const span: [number, number, number] = [env.max[0] - env.min[0], env.max[1] - env.min[1], env.max[2] - env.min[2]];
  const cellsAt = (s: number): number => Math.ceil(span[0] / s) * Math.ceil(span[1] / s) * Math.ceil(span[2] / s);
  let step = Math.max(1, res);
  while (cellsAt(step) > 2000) step *= 1.3;
  const seen = new Set<string>();
  for (let x = env.min[0]; x <= env.max[0]; x += step) {
    for (let y = env.min[1]; y <= env.max[1]; y += step) {
      for (let z = env.min[2]; z <= env.max[2]; z += step) {
        const c: Vec3 = [x, y, z];
        if (Math.abs(field(c)) > step * 1.2) continue;
        const p = projectToSurface(field, c);
        const key = `${Math.round(p[0])},${Math.round(p[1])},${Math.round(p[2])}`;
        if (seen.has(key)) continue;
        seen.add(key);
        const openN = scale(normalize(sdfNormal(field, p)), -1);
        out.push({ pos: p, up: openN, surface: classifySurface(openN) });
      }
    }
  }
  return out;
}

const dist = (a: Vec3, b: Vec3): number => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

export interface ScatterParams {
  field: Sdf;
  envelope: WorldBox;
  res: number;
  spaceId: string;
  socketPositions: Vec3[];
  required: RequiredAnchor[];
  kinds?: Record<string, ContentAnchorKind>;
  rng: Rng;
}

export function scatterSpace(p: ScatterParams): ContentAnchor[] {
  const kinds = p.kinds ?? DEFAULT_ANCHOR_KINDS;
  const candidates = collectCandidates(p.field, p.envelope, p.res);
  const accepted: ContentAnchor[] = [];
  let n = 0;

  const clears = (pos: Vec3, kind: ContentAnchorKind): boolean => {
    for (const s of p.socketPositions) if (dist(pos, s) < kind.clearanceFromStructural) return false;
    for (const a of accepted) {
      const other = kinds[a.kindId];
      const sep = Math.max(kind.minSeparation, other?.minSeparation ?? 0);
      if (dist(pos, a.pos) < sep) return false;
    }
    return true;
  };

  const place = (kind: ContentAnchorKind, tags: string[], binding: AnchorBinding | undefined, attempts: number): ContentAnchor | undefined => {
    for (let a = 0; a < attempts; a++) {
      const cand = candidates[p.rng.int(0, Math.max(0, candidates.length - 1))];
      if (!cand || !kind.allowedSurfaces.includes(cand.surface)) continue;
      const jitter: Vec3 = add(cand.pos, scale(cand.up, 0.1));
      if (!clears(jitter, kind)) continue;
      const anchor: ContentAnchor = { id: `${p.spaceId}:a${n++}`, spaceId: p.spaceId, kindId: kind.id, pos: cand.pos, up: cand.up, surface: cand.surface, tags };
      if (binding) anchor.binding = binding;
      accepted.push(anchor);
      return anchor;
    }
    return undefined;
  };

  // required first, raised attempt budget
  for (const req of p.required) {
    const kind = kinds[req.kindId];
    if (!kind) throw new GenError("anchors.unknown-kind", `unknown anchor kind "${req.kindId}"`, { kind: req.kindId, space: p.spaceId });
    const placed = place(kind, req.tags ?? [], req.binding, 64);
    if (!placed && !req.optional) {
      throw new GenError("anchors.required-unplaceable", `could not place required "${req.kindId}" anchor in Space "${p.spaceId}"`, { kind: req.kindId, space: p.spaceId });
    }
  }

  // decorative fill
  for (const kind of Object.values(kinds)) {
    if (kind.id === "gadget-pickup" || kind.id === "interactable" || kind.id === "landmark-feature" || kind.id === "vista") continue; // gameplay kinds come via required
    for (let i = 0; i < kind.targetDensity; i++) place(kind, [], undefined, 12);
  }

  return accepted;
}
