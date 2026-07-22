/**
 * Spline connectors — a Catmull-Rom curve between two sockets, realised as a
 * union of capsule SDFs (a smooth, non-Manhattan tube). The two ends leave along
 * each socket's outward direction so the corridor grows organically out of the
 * room and curves toward its partner.
 */

import type { Vec3 } from "../math/vec.js";
import { add, scale } from "../math/vec.js";
import { capsule, union, type Sdf } from "./sdf.js";

/** Catmull-Rom position at t∈[0,1] across control points (phantom endpoints). */
export function catmullRom(pts: readonly Vec3[], t: number): Vec3 {
  const n = pts.length;
  if (n === 1) return pts[0] as Vec3;
  const scaled = t * (n - 1);
  const i = Math.min(n - 2, Math.floor(scaled));
  const f = scaled - i;
  const p0 = pts[Math.max(0, i - 1)] as Vec3;
  const p1 = pts[i] as Vec3;
  const p2 = pts[i + 1] as Vec3;
  const p3 = pts[Math.min(n - 1, i + 2)] as Vec3;
  const f2 = f * f;
  const f3 = f2 * f;
  const cr = (a: number, b: number, c: number, d: number): number => 0.5 * (2 * b + (-a + c) * f + (2 * a - 5 * b + 4 * c - d) * f2 + (-a + 3 * b - 3 * c + d) * f3);
  return [cr(p0[0], p1[0], p2[0], p3[0]), cr(p0[1], p1[1], p2[1], p3[1]), cr(p0[2], p1[2], p2[2], p3[2])];
}

/** A tube SDF following a Catmull-Rom spline through `pts`. */
export function splineTube(pts: readonly Vec3[], radius: number, segments = 12): Sdf {
  const samples: Vec3[] = [];
  for (let i = 0; i <= segments; i++) samples.push(catmullRom(pts, i / segments));
  const caps: Sdf[] = [];
  for (let i = 0; i < samples.length - 1; i++) caps.push(capsule(samples[i] as Vec3, samples[i + 1] as Vec3, radius));
  return union(caps);
}

/** Build a connector tube between two sockets, leaving each along its outward direction. */
export function connectorTube(aPos: Vec3, aDir: Vec3, bPos: Vec3, bDir: Vec3, radius: number): Sdf {
  const dist = Math.hypot(bPos[0] - aPos[0], bPos[1] - aPos[1], bPos[2] - aPos[2]);
  const lead = Math.max(radius, dist * 0.3);
  const ctrl: Vec3[] = [aPos, add(aPos, scale(aDir, lead)), add(bPos, scale(bDir, lead)), bPos];
  return splineTube(ctrl, radius, 14);
}
