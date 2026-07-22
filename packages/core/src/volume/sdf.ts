/**
 * Signed distance fields. An `Sdf` maps a world point to a signed distance where
 * **negative = inside the open (walkable) volume** and **positive = solid rock**;
 * the room/area surface is the 0-isosurface the geometry pass contours. SDFs are
 * functions (used only during generation; the OUTPUT is meshed geometry, so they
 * never need to serialize). Pure arithmetic — deterministic.
 */

import type { Vec3 } from "../math/vec.js";
import { clamp, lerp } from "../math/curve.js";
import { fbm3 } from "../math/noise.js";

export type Sdf = (p: Vec3) => number;

export const sphere = (c: Vec3, r: number): Sdf => (p) => Math.hypot(p[0] - c[0], p[1] - c[1], p[2] - c[2]) - r;

export const ellipsoid = (c: Vec3, rad: Vec3): Sdf => (p) => {
  const nx = (p[0] - c[0]) / rad[0];
  const ny = (p[1] - c[1]) / rad[1];
  const nz = (p[2] - c[2]) / rad[2];
  const k = Math.hypot(nx, ny, nz);
  return (k - 1) * Math.min(rad[0], rad[1], rad[2]);
};

export const box = (c: Vec3, half: Vec3): Sdf => (p) => {
  const dx = Math.abs(p[0] - c[0]) - half[0];
  const dy = Math.abs(p[1] - c[1]) - half[1];
  const dz = Math.abs(p[2] - c[2]) - half[2];
  const ox = Math.max(dx, 0);
  const oy = Math.max(dy, 0);
  const oz = Math.max(dz, 0);
  return Math.hypot(ox, oy, oz) + Math.min(Math.max(dx, dy, dz), 0);
};

export const roundBox = (c: Vec3, half: Vec3, radius: number): Sdf => {
  const inner = box(c, [Math.max(0.01, half[0] - radius), Math.max(0.01, half[1] - radius), Math.max(0.01, half[2] - radius)]);
  return (p) => inner(p) - radius;
};

export const capsule = (a: Vec3, b: Vec3, r: number): Sdf => (p) => {
  const pax = p[0] - a[0];
  const pay = p[1] - a[1];
  const paz = p[2] - a[2];
  const bax = b[0] - a[0];
  const bay = b[1] - a[1];
  const baz = b[2] - a[2];
  const denom = bax * bax + bay * bay + baz * baz || 1e-6;
  const h = clamp((pax * bax + pay * bay + paz * baz) / denom, 0, 1);
  return Math.hypot(pax - bax * h, pay - bay * h, paz - baz * h) - r;
};

export const plane = (normal: Vec3, offset: number): Sdf => (p) => p[0] * normal[0] + p[1] * normal[1] + p[2] * normal[2] - offset;

// --- ops (open-space convention: min = union of open volumes) ---

export const union = (parts: readonly Sdf[]): Sdf => (p) => {
  let d = Infinity;
  for (const f of parts) d = Math.min(d, f(p));
  return d;
};

export const subtract = (a: Sdf, b: Sdf): Sdf => (p) => Math.max(a(p), -b(p));
export const intersect = (a: Sdf, b: Sdf): Sdf => (p) => Math.max(a(p), b(p));

export const smoothUnion = (k: number, parts: readonly Sdf[]): Sdf => (p) => {
  if (parts.length === 0) return Infinity;
  let d = (parts[0] as Sdf)(p);
  for (let i = 1; i < parts.length; i++) {
    const b = (parts[i] as Sdf)(p);
    const h = clamp(0.5 + (0.5 * (b - d)) / (k || 1e-6), 0, 1);
    d = lerp(b, d, h) - k * h * (1 - h);
  }
  return d;
};

/** Warp a base SDF with fractal noise to break the primitive silhouette (organic rock). */
export const displace = (base: Sdf, amp: number, freq: number, seed: number): Sdf => (p) => base(p) + amp * fbm3(p[0] * freq, p[1] * freq, p[2] * freq, seed);

/** Central-difference gradient (the surface normal at `p`), normalised. */
export function sdfNormal(f: Sdf, p: Vec3, eps = 0.05): Vec3 {
  const dx = f([p[0] + eps, p[1], p[2]]) - f([p[0] - eps, p[1], p[2]]);
  const dy = f([p[0], p[1] + eps, p[2]]) - f([p[0], p[1] - eps, p[2]]);
  const dz = f([p[0], p[1], p[2] + eps]) - f([p[0], p[1], p[2] - eps]);
  const len = Math.hypot(dx, dy, dz) || 1e-6;
  return [dx / len, dy / len, dz / len];
}
