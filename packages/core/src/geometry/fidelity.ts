/**
 * Fidelity — the PS2 look. Quantize a surface normal's spherical angles (azimuth
 * + elevation) to the 5° grid, so the naturalized surface becomes faceted at 5°
 * increments (flats/chamfers tend to 15°, radial shapes step by 5°). Deterministic
 * (uses our `datan2`/`dsin`/`dcos`, no host trig).
 */

import type { Vec3 } from "../math/vec.js";
import { datan2, dcos, dsin } from "../math/trig.js";

const STEP = Math.PI / 36; // 5°

export function quantizeNormal(n: Vec3): Vec3 {
  const len = Math.hypot(n[0], n[1], n[2]) || 1e-6;
  const nx = n[0] / len;
  const ny = n[1] / len;
  const nz = Math.max(-1, Math.min(1, n[2] / len));
  const az = datan2(ny, nx); // azimuth in XY
  const el = datan2(nz, Math.sqrt(Math.max(0, 1 - nz * nz))); // elevation = asin(nz)
  const qaz = Math.round(az / STEP) * STEP;
  const qel = Math.round(el / STEP) * STEP;
  const ce = dcos(qel);
  return [ce * dcos(qaz), ce * dsin(qaz), dsin(qel)];
}

/** Is a normal already on the 5° grid (within ε)? (test/verification helper) */
export function isQuantized(n: Vec3, eps = 1e-3): boolean {
  const q = quantizeNormal(n);
  return Math.abs(q[0] - n[0]) < eps && Math.abs(q[1] - n[1]) < eps && Math.abs(q[2] - n[2]) < eps;
}
