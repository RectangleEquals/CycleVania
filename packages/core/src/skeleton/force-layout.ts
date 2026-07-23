/**
 * Deterministic force-directed 3D layout — nodes settle where they "want" to sit:
 * springs pull connected nodes together, all pairs repel, a `zSeparation` bias
 * spreads verticality, and an optional per-node `targetZ` pulls toward the z-plan.
 * Seeded init + pure arithmetic (sqrt only) → byte-identical positions per seed.
 * Ported from the legacy layout; extended with the z-plan pull.
 */

import { Rng, type Vec3, type WorldBox } from "../math/index.js";

export interface LayoutNode {
  id: string;
  radius?: number;
  pinned?: Vec3;
  mass?: number;
  targetZ?: number; // z-plan target; pulled toward it if set
}

export interface LayoutEdge {
  a: string;
  b: string;
  rest?: number;
}

export interface LayoutOptions {
  seed: string | number;
  iterations?: number;
  spring?: number;
  repulsion?: number;
  damping?: number;
  spread?: number;
  zSeparation?: number;
  zPull?: number; // strength of the targetZ pull
  bounds?: WorldBox;
}

export interface LayoutResult {
  positions: Map<string, Vec3>;
}

interface Body {
  id: string; x: number; y: number; z: number;
  vx: number; vy: number; vz: number;
  fx: number; fy: number; fz: number;
  r: number; m: number; pinned: boolean; targetZ: number | undefined;
}

export function forceLayout(nodes: readonly LayoutNode[], edges: readonly LayoutEdge[], opts: LayoutOptions): LayoutResult {
  const iterations = opts.iterations ?? 220;
  const spring = opts.spring ?? 0.06;
  const repulsion = opts.repulsion ?? 40;
  const damping = opts.damping ?? 0.85;
  const spread = opts.spread ?? 12;
  const zSep = opts.zSeparation ?? 1.4;
  const zPull = opts.zPull ?? 0.05;
  const rng = new Rng(opts.seed);

  const bodies: Body[] = nodes.map((n) => {
    const r = n.radius ?? 2;
    if (n.pinned) return { id: n.id, x: n.pinned[0], y: n.pinned[1], z: n.pinned[2], vx: 0, vy: 0, vz: 0, fx: 0, fy: 0, fz: 0, r, m: n.mass ?? 1, pinned: true, targetZ: n.targetZ };
    return {
      id: n.id,
      x: (rng.next() - 0.5) * 2 * spread,
      y: (rng.next() - 0.5) * 2 * spread,
      z: n.targetZ !== undefined ? n.targetZ : (rng.next() - 0.5) * 2 * spread * 0.6 * zSep,
      vx: 0, vy: 0, vz: 0, fx: 0, fy: 0, fz: 0, r, m: n.mass ?? 1, pinned: false, targetZ: n.targetZ,
    };
  });
  const byId = new Map(bodies.map((b) => [b.id, b]));

  for (let iter = 0; iter < iterations; iter++) {
    for (const b of bodies) {
      b.fx = 0;
      b.fy = 0;
      b.fz = 0;
    }

    for (let i = 0; i < bodies.length; i++) {
      const a = bodies[i] as Body;
      for (let j = i + 1; j < bodies.length; j++) {
        const b = bodies[j] as Body;
        let dx = a.x - b.x;
        const dy = a.y - b.y;
        const dz = (a.z - b.z) * zSep;
        let d2 = dx * dx + dy * dy + dz * dz;
        if (d2 < 1e-4) {
          dx = (i - j) * 0.01 + 0.01;
          d2 = dx * dx + 1e-4;
        }
        const minSep = a.r + b.r;
        const d = Math.sqrt(d2);
        let f = repulsion / d2;
        if (d < minSep) f += (minSep - d) * spring * 4;
        const ux = dx / d;
        const uy = dy / d;
        const uz = dz / d;
        a.fx += ux * f; a.fy += uy * f; a.fz += uz * f;
        b.fx -= ux * f; b.fy -= uy * f; b.fz -= uz * f;
      }
    }

    for (const e of edges) {
      const a = byId.get(e.a);
      const b = byId.get(e.b);
      if (!a || !b) continue;
      const rest = e.rest ?? a.r + b.r + 4;
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const dz = b.z - a.z;
      const d = Math.sqrt(dx * dx + dy * dy + dz * dz) || 1e-4;
      const f = spring * (d - rest);
      const ux = dx / d;
      const uy = dy / d;
      const uz = dz / d;
      a.fx += ux * f; a.fy += uy * f; a.fz += uz * f;
      b.fx -= ux * f; b.fy -= uy * f; b.fz -= uz * f;
    }

    // z-plan pull
    for (const b of bodies) {
      if (b.pinned || b.targetZ === undefined) continue;
      b.fz += (b.targetZ - b.z) * zPull;
    }

    for (const b of bodies) {
      if (b.pinned) continue;
      b.vx = (b.vx + b.fx / b.m) * damping;
      b.vy = (b.vy + b.fy / b.m) * damping;
      b.vz = (b.vz + b.fz / b.m) * damping;
      const step = Math.sqrt(b.vx * b.vx + b.vy * b.vy + b.vz * b.vz);
      const cap = b.r + 2;
      if (step > cap) {
        const s = cap / step;
        b.vx *= s;
        b.vy *= s;
        b.vz *= s;
      }
      b.x += b.vx;
      b.y += b.vy;
      b.z += b.vz;
      if (opts.bounds) {
        b.x = Math.max(opts.bounds.min[0], Math.min(b.x, opts.bounds.max[0]));
        b.y = Math.max(opts.bounds.min[1], Math.min(b.y, opts.bounds.max[1]));
        b.z = Math.max(opts.bounds.min[2], Math.min(b.z, opts.bounds.max[2]));
      }
    }
  }

  // resolve residual overlaps deterministically (by id order)
  const sorted = [...bodies].sort((a, b) => (a.id < b.id ? -1 : 1));
  for (let pass = 0; pass < 4; pass++) {
    for (let i = 0; i < sorted.length; i++) {
      const a = sorted[i] as Body;
      for (let j = i + 1; j < sorted.length; j++) {
        const b = sorted[j] as Body;
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const dz = b.z - a.z;
        const d = Math.sqrt(dx * dx + dy * dy + dz * dz) || 1e-4;
        const minSep = a.r + b.r;
        if (d < minSep) {
          const push = (minSep - d) / 2;
          const ux = dx / d;
          const uy = dy / d;
          const uz = dz / d;
          if (!a.pinned) { a.x -= ux * push; a.y -= uy * push; a.z -= uz * push; }
          if (!b.pinned) { b.x += ux * push; b.y += uy * push; b.z += uz * push; }
        }
      }
    }
  }

  const round = (v: number): number => Math.round(v * 1000) / 1000;
  const positions = new Map<string, Vec3>();
  for (const b of bodies) positions.set(b.id, [round(b.x), round(b.y), round(b.z)]);
  return { positions };
}
