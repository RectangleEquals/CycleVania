/**
 * The AreaComposer (L2) — one Region → one Area of positioned Spaces + intra-area
 * connectors + boundary sockets. Decides ALL abstract Space facts here: outdoor,
 * junction insertion for connectivity capacity, recipe-envelope reservation,
 * secret bias, z-plan. Force-directed layout gives organic positions. Nothing
 * here builds a hull — that's L3.
 */

import { Rng, add, sub, scale, normalize, length, type Vec3 } from "../math/index.js";
import type { NodeRole, RegionId } from "../graph/index.js";
import type { Traversal } from "../types.js";
import { deriveAreaDials, type AreaDialConfig, type SpaceBudget } from "./space-budget.js";
import { forceLayout, type LayoutEdge, type LayoutNode } from "./force-layout.js";
import { envelopeFor, spaceRadius, type AreaSkeleton, type ConnectorKind, type ConnectorSpec, type ProvisionalSocket, type SpaceSpec } from "./space-plan.js";

export const DEFAULT_DEGREE: Record<string, { min: number; max: number }> = {
  hub: { min: 3, max: 6 },
  segment: { min: 2, max: 3 },
  gate: { min: 2, max: 3 },
  vault: { min: 1, max: 1 },
  capstone: { min: 2, max: 4 },
  terminal: { min: 1, max: 2 },
  junction: { min: 2, max: 3 },
};

const LENGTH_BOUNDS: Record<Traversal, { min: number; max: number }> = {
  walk: { min: 4, max: 30 },
  crawl: { min: 2, max: 10 },
  drop: { min: 2, max: 20 },
  climb: { min: 2, max: 16 },
  swim: { min: 4, max: 24 },
  vertical: { min: 2, max: 20 },
};

export interface ComposeAreaParams {
  regionId: RegionId;
  role: NodeRole;
  budgetSlice: number;
  buckets: Readonly<Record<string, number>>;
  biome: string;
  incidentEdges: number;
  reservedRecipes: string[];
  dialCfg: AreaDialConfig;
  rng: Rng;
}

function degreeOf(role: string): { min: number; max: number } {
  return DEFAULT_DEGREE[role] ?? { min: 2, max: 3 };
}

export function composeArea(p: ComposeAreaParams): AreaSkeleton {
  const relaxations: string[] = [];
  const dials = deriveAreaDials(p.budgetSlice, p.buckets, p.role, p.dialCfg, p.rng.fork("dials"));
  const planRng = p.rng.fork("plan");

  // --- 1. how many spaces (enough to host incident edges + reservations) ---
  let spaceCount = Math.max(dials.spaceCount, p.reservedRecipes.length, 1);
  // capacity: total boundary sockets available must cover incident edges.
  const spaceRoleOf = (i: number): NodeRole | "junction" =>
    i === 0 ? (p.role === "hub" ? "hub" : "segment") : p.role === "capstone" && i === 1 ? "capstone" : "segment";
  let capacity = 0;
  for (let i = 0; i < spaceCount; i++) capacity += degreeOf(spaceRoleOf(i)).max;
  while (capacity < p.incidentEdges) {
    spaceCount++;
    capacity += degreeOf("junction").max;
    relaxations.push("skeleton.junction-inserted");
  }

  // --- 2. create spaces ---
  const share = p.budgetSlice / spaceCount;
  const spaces: SpaceSpec[] = [];
  for (let i = 0; i < spaceCount; i++) {
    const isJunction = i >= dials.spaceCount && i >= Math.max(dials.spaceCount, p.reservedRecipes.length);
    const role: NodeRole | "junction" = isJunction ? "junction" : spaceRoleOf(i);
    const large = planRng.chance(dials.largeSpaceChance);
    const volumeCells = Math.max(8, Math.round(share * (large ? 1.8 : 1)));
    const outdoor = (role === "segment" || role === "hub") && large && planRng.chance(dials.outdoorChance);
    const hidden = i > 0 && planRng.chance(dials.secretFraction);
    const budget: SpaceBudget = { volumeCells, polyAllowance: volumeCells * 40, degree: degreeOf(role) };
    spaces.push({
      id: `${p.regionId}:s${i}`,
      kind: outdoor ? "outdoor" : "room",
      role,
      regionId: p.regionId,
      budget,
      origin: [0, 0, 0],
      envelope: envelopeFor([0, 0, 0], volumeCells),
      biome: p.biome,
      outdoor,
      landmark: false,
      reservedRecipes: [],
      hidden,
    });
  }
  // reserve recipe envelopes on the first eligible room spaces
  let ri = 0;
  for (const recipe of p.reservedRecipes) {
    const target = spaces.find((s, idx) => idx >= ri && !s.outdoor) ?? spaces[0];
    if (target) {
      target.reservedRecipes.push(recipe);
      ri = spaces.indexOf(target) + 1;
    }
  }

  // --- 3. intra-area connectivity: a spanning path + loop edges ---
  const intraEdges: { a: number; b: number }[] = [];
  for (let i = 1; i < spaceCount; i++) intraEdges.push({ a: i - 1, b: i });
  const loopAttempts = Math.round(dials.loopDensity * spaceCount);
  for (let a = 0; a < loopAttempts && spaceCount >= 3; a++) {
    if (!planRng.chance(dials.loopDensity)) continue;
    const i = planRng.int(2, spaceCount - 1);
    const j = planRng.int(0, i - 2);
    if (!intraEdges.some((e) => (e.a === i && e.b === j) || (e.a === j && e.b === i))) intraEdges.push({ a: i, b: j });
  }

  // --- 4. z-plan + force layout ---
  const layoutNodes: LayoutNode[] = spaces.map((s, i) => ({
    id: s.id,
    radius: spaceRadius(s.budget.volumeCells),
    ...(i === 0 ? { pinned: [0, 0, 0] as Vec3 } : {}),
    targetZ: spaceCount <= 1 ? 0 : (i / (spaceCount - 1) - 0.5) * dials.zSpread,
  }));
  const layoutEdges: LayoutEdge[] = intraEdges.map((e) => ({ a: spaces[e.a]?.id ?? "", b: spaces[e.b]?.id ?? "" }));
  const { positions } = forceLayout(layoutNodes, layoutEdges, { seed: `${p.regionId}:layout`, zSeparation: 1 + dials.zSpread * 0.1 });
  for (const s of spaces) {
    const pos = positions.get(s.id);
    if (pos) {
      s.origin = pos;
      s.envelope = envelopeFor(pos, s.budget.volumeCells);
    }
  }

  // --- 5. sockets + intra connectors from the layout ---
  const connectors: ConnectorSpec[] = [];
  const sockets: ProvisionalSocket[] = [];
  const usedDegree = new Map<string, number>();
  let socketN = 0;
  const mkSocket = (space: SpaceSpec, dir: Vec3, traversal: Traversal, signature: string): ProvisionalSocket => {
    const r = spaceRadius(space.budget.volumeCells);
    const s: ProvisionalSocket = {
      id: `${space.id}:k${socketN++}`,
      spaceId: space.id,
      pos: add(space.origin, scale(dir, r)),
      dir,
      kind: "structural",
      traversal,
      signature,
    };
    sockets.push(s);
    usedDegree.set(space.id, (usedDegree.get(space.id) ?? 0) + 1);
    return s;
  };
  for (const e of intraEdges) {
    const A = spaces[e.a];
    const B = spaces[e.b];
    if (!A || !B) continue;
    const delta = sub(B.origin, A.origin);
    const dir = normalize(delta);
    const dist = length(delta);
    const dz = delta[2];
    const thr = 0.6 * (spaceRadius(A.budget.volumeCells) + spaceRadius(B.budget.volumeCells));
    const traversal: Traversal = dz < -thr ? "drop" : dz > thr ? "climb" : "walk";
    const kind: ConnectorKind =
      traversal === "drop" || traversal === "climb" ? "shaft" : A.outdoor !== B.outdoor ? "open-seam" : planRng.chance(0.4) ? "curved" : "straight";
    const sig = `${kind}|${traversal}`;
    const sa = mkSocket(A, dir, traversal, sig);
    const sb = mkSocket(B, scale(dir, -1), traversal, sig);
    sa.partner = { spaceId: B.id, socketId: sb.id };
    sb.partner = { spaceId: A.id, socketId: sa.id };
    const bounds = LENGTH_BOUNDS[traversal];
    const conn: ConnectorSpec = {
      id: `${p.regionId}:c${connectors.length}`,
      from: { spaceId: A.id, socketId: sa.id },
      to: { spaceId: B.id, socketId: sb.id },
      kind,
      traversal,
      lengthBounds: bounds,
    };
    if (dist > bounds.max) conn.waypoints = [scale(add(A.origin, B.origin), 0.5)];
    connectors.push(conn);
  }

  // --- 6. boundary sockets for inter-area edges (round-robin, respecting degree) ---
  const boundarySockets: ProvisionalSocket[] = [];
  let cursor = 0;
  const dirs: Vec3[] = [
    [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [1, 1, 0], [-1, -1, 0], [1, -1, 0], [-1, 1, 0],
  ];
  for (let e = 0; e < p.incidentEdges; e++) {
    let placed = false;
    for (let tries = 0; tries < spaces.length; tries++) {
      const s = spaces[(cursor + tries) % spaces.length];
      if (!s) continue;
      if ((usedDegree.get(s.id) ?? 0) < s.budget.degree.max) {
        const dir = normalize(dirs[boundarySockets.length % dirs.length] as Vec3);
        boundarySockets.push(mkSocket(s, dir, "walk", "arch|walk"));
        cursor = (cursor + tries + 1) % spaces.length;
        placed = true;
        break;
      }
    }
    if (!placed) {
      relaxations.push("skeleton.boundary-overflow");
      const s = spaces[0];
      if (s) boundarySockets.push(mkSocket(s, [1, 0, 0], "walk", "arch|walk"));
    }
  }

  return {
    regionId: p.regionId,
    role: p.role,
    spaces,
    connectors,
    sockets,
    boundarySockets,
    entrySpaceId: spaces[0]?.id ?? `${p.regionId}:s0`,
    relaxations,
  };
}
