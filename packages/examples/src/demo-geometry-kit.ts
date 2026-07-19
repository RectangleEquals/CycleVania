/**
 * A demo GeometryKit in the two-tier retro style: PS2-era environment pieces with
 * chamfers, bevels, curves and 45° cuts (the look we WANT — not pure 90° boxes),
 * plus Z-ready non-boxy pieces (ramps, climb-faces, drop-hatches, crawlspaces)
 * registered for future vertical/snake layouts. Ids are opaque handles the parent
 * maps to actual meshes.
 */

import type { GeometryKit } from "@cyclevania/core";

const A = (deg: number): number => (deg * Math.PI) / 180;
/** PS2 angle palette: 15° increments (the 5° grid's typical step for flats/chamfers). */
const PS2 = Array.from({ length: 24 }, (_, i) => A(i * 15));

export const demoGeometryKit: GeometryKit = {
  pieces: [
    // floors & ceilings
    { id: "floor-slab", role: "floor", snapAngles: [0], tags: ["floor", "ps2"], collider: "solid" },
    { id: "floor-tiled", role: "floor", snapAngles: [0], tags: ["floor", "ornate"], collider: "solid" },
    { id: "ceiling-vault", role: "ceiling", snapAngles: [0], tags: ["ceiling", "vaulted"], collider: "solid" },
    { id: "ceiling-beamed", role: "ceiling", snapAngles: [0], tags: ["ceiling", "beamed"], collider: "solid" },
    // walls — flat, chamfered, buttressed (PS2 bevel/curve intent)
    { id: "wall-flat", role: "wall", snapAngles: PS2, tags: ["wall"], collider: "solid" },
    { id: "wall-chamfer", role: "wall", snapAngles: PS2, tags: ["wall", "chamfer"], collider: "solid" },
    { id: "wall-buttress", role: "wall", snapAngles: PS2, tags: ["wall", "buttress"], collider: "solid" },
    // corners — bevelled & curved (not right-angle)
    { id: "corner-bevel", role: "corner", snapAngles: PS2, tags: ["corner", "bevel"], collider: "solid" },
    { id: "corner-curved", role: "corner", snapAngles: PS2, tags: ["corner", "curved"], collider: "solid" },
    // openings (not always a door)
    { id: "arch-door", role: "opening", snapAngles: PS2, traversal: "walk", socketCapable: true, tags: ["opening", "arch"], collider: "none" },
    { id: "cave-mouth", role: "opening", snapAngles: PS2, traversal: "walk", socketCapable: true, tags: ["opening", "cave"], collider: "none" },
    // Z-ready / non-boxy (registered for the Phase B vertical & snake layouts)
    { id: "ramp-45", role: "ramp45", snapAngles: PS2, traversal: "walk", tags: ["ramp", "ps2"], collider: "solid" },
    { id: "curved-hall", role: "curved", snapAngles: PS2, traversal: "walk", tags: ["curved"], collider: "solid" },
    { id: "climb-face", role: "climb-face", snapAngles: PS2, traversal: "climb", socketCapable: true, tags: ["climb"], collider: "solid" },
    { id: "drop-hatch", role: "drop-hatch", snapAngles: [0], traversal: "drop", socketCapable: true, tags: ["drop"], collider: "none" },
    { id: "crawlspace", role: "crawlspace", snapAngles: PS2, traversal: "crawl", socketCapable: true, tags: ["crawl"], collider: "none" },
    { id: "open-edge", role: "open", snapAngles: PS2, traversal: "open", socketCapable: true, tags: ["outdoor"], collider: "none" },
  ],
};
