/**
 * Socket resolution (provisional → resolved). Sphere-march the owning Space's hull
 * field inward from just outside the envelope until the open surface, then build an
 * orthonormal basis (forward into the open volume, up ≈ world-up), fidelity-snapped
 * so apertures agree with the faceting. Decoupled from skeleton types — takes
 * primitives, returns a pose.
 */

import { add, scale, cross, dot, normalize, quantizeNormal, type Vec3 } from "../math/index.js";
import { sdfNormal, type Sdf } from "./sdf.js";

export interface SocketBasis {
  forward: Vec3;
  up: Vec3;
  right: Vec3;
}

export interface ResolvedPose {
  pos: Vec3;
  basis: SocketBasis;
  passable: boolean;
}

const WORLD_UP: Vec3 = [0, 0, 1];

function bisect(f: Sdf, a: Vec3, b: Vec3, iters: number): Vec3 {
  let lo = a;
  let hi = b;
  for (let i = 0; i < iters; i++) {
    const mid: Vec3 = [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2];
    if (f(mid) > 0) lo = mid;
    else hi = mid;
  }
  return [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2];
}

export function resolveSocketPose(
  pos: Vec3,
  dir: Vec3,
  hull: Sdf,
  angleStepDeg: number | null,
  radius: number,
  outMargin = 4,
  minStep = 0.5,
): ResolvedPose {
  const inward = scale(normalize(dir), -1);
  let p: Vec3 = add(pos, scale(normalize(dir), outMargin));
  let prev = hull(p);
  let hit = p;
  for (let i = 0; i < 128; i++) {
    const step = Math.max(Math.abs(prev), minStep);
    const np = add(p, scale(inward, step));
    const d = hull(np);
    if ((prev > 0 && d <= 0) || Math.abs(d) < 1e-3) {
      hit = prev > 0 && d <= 0 ? bisect(hull, p, np, 16) : np;
      break;
    }
    p = np;
    prev = d;
    hit = np;
  }

  // forward faces into the open volume (−gradient), up ≈ world-up projected off forward
  let forward = scale(sdfNormal(hull, hit), -1);
  forward = normalize(forward);
  let up: Vec3 = [WORLD_UP[0] - forward[0] * dot(WORLD_UP, forward), WORLD_UP[1] - forward[1] * dot(WORLD_UP, forward), WORLD_UP[2] - forward[2] * dot(WORLD_UP, forward)];
  if (Math.hypot(up[0], up[1], up[2]) < 1e-3) up = Math.abs(forward[0]) < 0.9 ? [1, 0, 0] : [0, 1, 0];
  up = normalize(up);
  const right = normalize(cross(up, forward));

  const basis: SocketBasis = {
    forward: quantizeNormal(forward, angleStepDeg),
    up: quantizeNormal(up, angleStepDeg),
    right: quantizeNormal(right, angleStepDeg),
  };
  // passable: a point half a radius into the open direction is open (bisect can
  // land a hair on the solid side, so test the open side, not the surface point).
  const passable = hull(add(hit, scale(forward, radius * 0.5))) < 0;
  return { pos: hit, basis, passable };
}
