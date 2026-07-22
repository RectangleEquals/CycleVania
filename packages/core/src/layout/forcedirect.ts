/**
 * Deterministic force-directed 3D graph layout (Gemini L2). Places graph nodes where
 * they "want" to sit: springs pull connected nodes together, all pairs repel, and a
 * `zSeparation` bias spreads nodes vertically (verticality-first). Seeded init + pure
 * arithmetic (sqrt only, no host trig/random) → byte-identical positions per seed.
 */

import type { Vec3, WorldBox } from "../math/vec.js";
import { Rng } from "../math/rng.js";

export interface LayoutNode {
  id: string;
  radius?: number; // bounding radius (repulsion + overlap spacing)
  pinned?: Vec3; // fixed position (won't move)
  mass?: number;
}

export interface LayoutEdge {
  a: string;
  b: string;
  rest?: number; // desired edge length (defaults from node radii)
}

export interface LayoutOptions {
  seed: string | number;
  iterations?: number;
  spring?: number; // edge stiffness
  repulsion?: number; // node repulsion strength
  damping?: number; // velocity retention per step
  spread?: number; // initial scatter radius
  zSeparation?: number; // >1 amplifies vertical spread (verticality-first)
  bounds?: WorldBox;
}

export interface LayoutResult {
  positions: Map<string, Vec3>;
}

interface Body {
  id: string;
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
  fx: number;
  fy: number;
  fz: number;
  r: number;
  m: number;
  pinned: boolean;
}

export function forceLayout(nodes: readonly LayoutNode[], edges: readonly LayoutEdge[], opts: LayoutOptions): LayoutResult {
  const iterations = opts.iterations ?? 240;
  const spring = opts.spring ?? 0.08;
  const repulsion = opts.repulsion ?? 40;
  const damping = opts.damping ?? 0.85;
  const spread = opts.spread ?? 12;
  const zSep = opts.zSeparation ?? 1.4;
  const rng = new Rng(opts.seed);

  const bodies: Body[] = nodes.map((n) => {
    const r = n.radius ?? 2;
    if (n.pinned) return { id: n.id, x: n.pinned[0], y: n.pinned[1], z: n.pinned[2], vx: 0, vy: 0, vz: 0, fx: 0, fy: 0, fz: 0, r, m: n.mass ?? 1, pinned: true };
    // seeded scatter in a box, z pre-stretched for vertical separation
    return {
      id: n.id,
      x: (rng.next() - 0.5) * 2 * spread,
      y: (rng.next() - 0.5) * 2 * spread,
      z: (rng.next() - 0.5) * 2 * spread * 0.6 * zSep,
      vx: 0,
      vy: 0,
      vz: 0,
      fx: 0,
      fy: 0,
      fz: 0,
      r,
      m: n.mass ?? 1,
      pinned: false,
    };
  });
  const byId = new Map(bodies.map((b) => [b.id, b]));

  for (let iter = 0; iter < iterations; iter++) {
    for (const b of bodies) {
      b.fx = 0;
      b.fy = 0;
      b.fz = 0;
    }

    // pairwise repulsion (O(n²); node counts are small)
    for (let i = 0; i < bodies.length; i++) {
      const a = bodies[i] as Body;
      for (let j = i + 1; j < bodies.length; j++) {
        const b = bodies[j] as Body;
        let dx = a.x - b.x;
        const dy = a.y - b.y;
        const dz = (a.z - b.z) * zSep; // amplify vertical separation
        let d2 = dx * dx + dy * dy + dz * dz;
        if (d2 < 1e-4) {
          dx = (i - j) * 0.01 + 0.01;
          d2 = dx * dx + 1e-4;
        }
        const minSep = a.r + b.r;
        const d = Math.sqrt(d2);
        // Coulomb + hard overlap kick when closer than combined radii
        let f = repulsion / d2;
        if (d < minSep) f += (minSep - d) * spring * 4;
        const ux = dx / d;
        const uy = dy / d;
        const uz = dz / d;
        a.fx += ux * f;
        a.fy += uy * f;
        a.fz += uz * f;
        b.fx -= ux * f;
        b.fy -= uy * f;
        b.fz -= uz * f;
      }
    }

    // springs along edges
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
      a.fx += ux * f;
      a.fy += uy * f;
      a.fz += uz * f;
      b.fx -= ux * f;
      b.fy -= uy * f;
      b.fz -= uz * f;
    }

    // integrate
    for (const b of bodies) {
      if (b.pinned) continue;
      b.vx = (b.vx + b.fx / b.m) * damping;
      b.vy = (b.vy + b.fy / b.m) * damping;
      b.vz = (b.vz + b.fz / b.m) * damping;
      // clamp step so a single spike can't explode the layout
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

  const positions = new Map<string, Vec3>();
  for (const b of bodies) positions.set(b.id, [b.x, b.y, b.z]);
  return { positions };
}
