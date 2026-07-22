/**
 * Area geometry pass (Gemini L3+L4) — the volumetric backend. Runs AFTER the room
 * layout: places an organic SDF hull per room (sized to its bounds, biome-picked),
 * spline-tubes for connectors between real socket world-positions, composes the area
 * field, then dual-contours it at 5° fidelity into a modular, world-grid-aligned
 * GeneratedKit + instances, plus a serializable occupancy grid + dressing anchors.
 *
 * Additive: the abstract `cells` stay for the fallback/inspector/sim; this attaches
 * `kit`/`instances`/`occupancy`/`dressing` to the AreaDescriptor. Gated by a flag so
 * bulk solvability soaks stay fast.
 */

import { Rng, fnv1a } from "../math/rng.js";
import type { Vec3 } from "../math/vec.js";
import { boxUnionAll } from "../math/geom.js";
import { hull, connectorTube, composeAreaField, type Sdf } from "../volume/index.js";
import { dualContour, meshToKit, occupancyGrid, dressArea } from "../geometry/index.js";
import type { Registry } from "../registries/registry.js";
import type { HullArchetypeDef } from "../registries/hull-archetypes.js";
import type { AreaDescriptor } from "../descriptors/descriptor.js";
import type { Coord } from "../spatial/grid.js";

/** Max cells along any axis — keeps the voxel grid (and thus cost) bounded. */
const MAX_DIM = 72;

function pickArchetype(reg: Registry, kind: string, outdoorChance: number, rng: Rng): HullArchetypeDef {
  const all = Object.values(reg.hullArchetypes);
  const suited = all.filter((a) => !a.roomKinds || a.roomKinds.includes(kind));
  const pool = suited.length > 0 ? suited : all;
  // budget-scaled chance to allow an occasional outdoor/large archetype
  const wantOutdoor = rng.chance(outdoorChance);
  const weighted = pool.map((a) => ({ a, w: Math.max(0.01, (a.weight ?? 1) * (a.outdoor ? (wantOutdoor ? 2.5 : 0.15) : 1)) }));
  const total = weighted.reduce((s, x) => s + x.w, 0);
  let r = rng.next() * total;
  for (const x of weighted) {
    r -= x.w;
    if (r <= 0) return x.a;
  }
  return pool[0] as HullArchetypeDef;
}

/** Build the area's generated geometry and attach it to the descriptor (mutates `area`). */
export function buildAreaGeometry(area: AreaDescriptor, reg: Registry, seed: string): void {
  if (area.rooms.length === 0) return;
  const rng = new Rng(`${seed}:geom`);
  const biome = area.biome ?? reg.defaultBiome.id;
  const noiseAmp = reg.defaultBiome.noise?.amp ?? 0.3;

  // --- one organic hull per room, sized to (a little inside) its bounds ---
  const hulls: Sdf[] = [];
  const outdoorChance = 0.12 + 0.25 * (area.role === "segment" ? 1 : 0.3);
  for (const room of area.rooms) {
    const b = room.bounds;
    const center: Vec3 = [(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2];
    const size: Vec3 = [(b.max[0] - b.min[0]) * 0.9, (b.max[1] - b.min[1]) * 0.9, (b.max[2] - b.min[2]) * 0.9];
    const arch = pickArchetype(reg, room.kind, outdoorChance, rng);
    const hseed = fnv1a(`${seed}:${room.nodeId}`);
    const params = { center, size, seed: hseed, noise: (arch.noise ?? 0.3) * (0.6 + noiseAmp) };
    hulls.push(arch.sdf ? arch.sdf(params) : hull(arch.archetype ?? "cavern", params));
    if (arch.outdoor) room.outdoor = true;
    room.biome = biome;
  }

  // --- spline connectors between the real socket world-positions ---
  const connectors: Sdf[] = [];
  const socketOf = (roomId: string, sockId: string): { pos: Vec3; dir: Vec3 } | undefined => {
    const room = area.rooms.find((r) => r.nodeId === roomId);
    const s = room?.sockets.find((x) => x.id === sockId);
    return s ? { pos: s.pos, dir: s.dir } : undefined;
  };
  for (const c of area.connectors) {
    const a = socketOf(c.fromRoom, c.fromSocket);
    const b = socketOf(c.toRoom, c.toSocket);
    if (!a || !b) continue;
    const radius = c.kind === "vertical" ? 2 : 1.6;
    connectors.push(connectorTube(a.pos, a.dir, b.pos, b.dir, radius));
  }

  const field = composeAreaField(hulls, connectors, 1.6);

  // --- voxel bounds: area bounds padded by a rock margin, resolution clamped ---
  const bounds = boxUnionAll(area.rooms.map((r) => r.bounds));
  const pad = 3;
  const origin: Vec3 = [bounds.min[0] - pad, bounds.min[1] - pad, bounds.min[2] - pad];
  const span: Vec3 = [bounds.max[0] - bounds.min[0] + pad * 2, bounds.max[1] - bounds.min[1] + pad * 2, bounds.max[2] - bounds.min[2] + pad * 2];
  let res = reg.fidelity.voxelRes;
  let dims: Coord = [Math.ceil(span[0] / res), Math.ceil(span[1] / res), Math.ceil(span[2] / res)];
  const worst = Math.max(dims[0], dims[1], dims[2]);
  if (worst > MAX_DIM) {
    res = res * (worst / MAX_DIM);
    dims = [Math.ceil(span[0] / res), Math.ceil(span[1] / res), Math.ceil(span[2] / res)];
  }

  // --- naturalize → 5° fidelity → dual contour → modular kit ---
  const mesh = dualContour(field.sdf, origin, dims, res, reg.fidelity.snapNormals);
  const { kit, instances } = meshToKit(mesh, origin, res, { biome });
  const occ = occupancyGrid(field.sdf, origin, dims, res);
  const dressing = dressArea(occ, fnv1a(`${seed}:dress`));

  area.kit = kit;
  area.instances = instances;
  area.occupancy = { origin: occ.origin, res: occ.res, dims: occ.dims, solid: Array.from(occ.solid) };
  area.dressing = dressing;
  area.biome = biome;
}
