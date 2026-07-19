/**
 * ConnectorComposer — turns the abstract link between two room sockets into an
 * actual traversable corridor with real geometry:
 *   · lateral links  → a Manhattan corridor whose floor **ramps** across its run
 *     when the two ends differ in height (stairs/ramp, not a broken vertical column);
 *   · stacked links  → a vertical **shaft** (a climbable tube of walls) between the
 *     two rooms.
 * The corridor carries its own origin so a consumer renders its cells like a room's.
 */

import { Rng } from "../math/rng.js";
import { piecesForRole } from "../registries/geometry-kit.js";
import type { Registry } from "../registries/registry.js";
import type { Vec3 } from "../math/vec.js";
import type { CellDescriptor } from "../descriptors/descriptor.js";
import type { Coord } from "../spatial/grid.js";
import type { Traversal } from "../types.js";

export interface ConnectorGeom {
  origin: Vec3;
  cellSize: number;
  cells: CellDescriptor[];
  /** Dominant traversal the corridor implies (walk for lateral, climb for a shaft). */
  traversal: Traversal;
}

export function corridorGeometry(aPos: Vec3, bPos: Vec3, registry: Registry, seed: string | number): ConnectorGeom {
  const cs = registry.grid.roomCellSize;
  const rng = new Rng(seed);
  const kit = registry.geometryKit;
  const pick = (role: string): string | null => {
    const ps = piecesForRole(kit, role);
    return ps.length ? (ps[Math.floor(rng.next() * ps.length)] as { id: string }).id : null;
  };
  const g = (v: Vec3): [number, number, number] => [Math.round(v[0] / cs), Math.round(v[1] / cs), Math.round(v[2] / cs)];
  const ga = g(aPos);
  const gb = g(bPos);

  const roleAt = new Map<string, string>();
  const put = (c: [number, number, number], role: string): void => {
    const k = c.join(",");
    const cur = roleAt.get(k);
    if (!cur || (cur === "wall" && role !== "wall")) roleAt.set(k, role);
  };

  const dz = gb[2] - ga[2];
  const pureVertical = ga[0] === gb[0] && ga[1] === gb[1] && dz !== 0;
  let traversal: Traversal = "walk";

  if (pureVertical) {
    // a climbable shaft between two stacked rooms
    traversal = "climb";
    const x = ga[0];
    const y = ga[1];
    const z0 = Math.min(ga[2], gb[2]);
    const z1 = Math.max(ga[2], gb[2]);
    for (let z = z0; z <= z1; z++) {
      put([x, y, z], "opening"); // the climbable shaft interior
      put([x + 1, y, z], "wall");
      put([x - 1, y, z], "wall");
      put([x, y + 1, z], "wall");
      put([x, y - 1, z], "wall");
    }
    put([x, y, z0 - 1], "floor");
    put([x, y, z1 + 1], "ceiling");
  } else {
    // lateral corridor: Manhattan (x then y) with the floor height RAMPED across the run
    if (dz !== 0) traversal = "walk"; // a ramp is still walkable
    const hpath: Array<[number, number]> = [[ga[0], ga[1]]];
    let hx = ga[0];
    let hy = ga[1];
    while (hx !== gb[0]) {
      hx += hx < gb[0] ? 1 : -1;
      hpath.push([hx, hy]);
    }
    while (hy !== gb[1]) {
      hy += hy < gb[1] ? 1 : -1;
      hpath.push([hx, hy]);
    }
    const n = Math.max(1, hpath.length - 1);
    const path: Array<[number, number, number]> = hpath.map(([x, y], i) => [x, y, ga[2] + Math.round((dz * i) / n)]);
    for (let i = 0; i < path.length; i++) {
      const p = path[i] as [number, number, number];
      put([p[0], p[1], p[2]], "floor");
      put([p[0], p[1], p[2] + 2], "ceiling");
      const prev = path[i - 1] ?? p;
      const next = path[i + 1] ?? p;
      const ddx = next[0] - prev[0];
      const ddy = next[1] - prev[1];
      if (Math.abs(ddx) >= Math.abs(ddy)) {
        put([p[0], p[1] + 1, p[2] + 1], "wall");
        put([p[0], p[1] - 1, p[2] + 1], "wall");
      } else {
        put([p[0] + 1, p[1], p[2] + 1], "wall");
        put([p[0] - 1, p[1], p[2] + 1], "wall");
      }
    }
  }

  let mnx = Infinity;
  let mny = Infinity;
  let mnz = Infinity;
  for (const k of roleAt.keys()) {
    const [x, y, z] = k.split(",").map(Number) as [number, number, number];
    mnx = Math.min(mnx, x);
    mny = Math.min(mny, y);
    mnz = Math.min(mnz, z);
  }
  const origin: Vec3 = [mnx * cs, mny * cs, mnz * cs];
  const cells: CellDescriptor[] = [];
  for (const [k, role] of roleAt) {
    const [x, y, z] = k.split(",").map(Number) as [number, number, number];
    const coord: Coord = [x - mnx, y - mny, z - mnz];
    cells.push({ coord, role, kitId: pick(role), yaw: 0, sockets: [], contents: [] });
  }
  return { origin, cellSize: cs, cells, traversal };
}
