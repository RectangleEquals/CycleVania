/**
 * Normal quantization for the fidelity spectrum — snap a unit normal to the
 * nearest orientation on an angular grid (in spherical coords, via deterministic
 * trig). `angleStepDeg`: 90 (orthogonal), 45, 15, 5 (PS2 faceting), or null (no
 * snap = smooth). Used by L3 socket bases and the L4 mesher. Quantizing the
 * hermite normals BEFORE vertex placement is what produces coherent facets.
 */

import { datan2, dcos, dsin } from "./trig.js";
import { normalize, type Vec3 } from "./vec.js";

export function quantizeNormal(n: Vec3, angleStepDeg: number | null, snap = true): Vec3 {
  const u = normalize(n);
  if (!snap || angleStepDeg === null || angleStepDeg <= 0) return u;
  const STEP = (angleStepDeg * Math.PI) / 180;
  const az = datan2(u[1], u[0]);
  const el = datan2(u[2], Math.hypot(u[0], u[1]));
  const qaz = Math.round(az / STEP) * STEP;
  const qel = Math.round(el / STEP) * STEP;
  const cel = dcos(qel);
  return normalize([cel * dcos(qaz), cel * dsin(qaz), dsin(qel)]);
}

/** Is `n` already (within eps) on the `angleStepDeg` grid? */
export function isQuantized(n: Vec3, angleStepDeg: number, eps = 1e-3): boolean {
  const q = quantizeNormal(n, angleStepDeg, true);
  const u = normalize(n);
  return Math.abs(q[0] - u[0]) <= eps && Math.abs(q[1] - u[1]) <= eps && Math.abs(q[2] - u[2]) <= eps;
}
