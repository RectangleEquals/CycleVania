/**
 * Demo biome content-packs + hull archetypes + a game-authored LANDMARK set-piece —
 * the HYBRID hull data the geometry pass samples. This is EXAMPLE game content: a real
 * game ships its own richer packs. Two biomes match the two demo styles.
 */

import type { BiomePack, BiomeRegistry, HullArchetypeRegistry } from "@cyclevania/core";
import { ellipsoid, capsule, box, subtract, smoothUnion, displace, type Sdf } from "@cyclevania/core";
import type { Vec3 } from "@cyclevania/core";

/** A memorable central-atrium landmark: a domed open volume around a solid pillar. */
function grandAtrium(params: { center: Vec3; size: Vec3; seed: number; noise?: number }): Sdf {
  const { center, size, seed } = params;
  const half: Vec3 = [size[0] / 2, size[1] / 2, size[2] / 2];
  const dome = ellipsoid(center, half); // open interior (negative inside)
  const floor = box([center[0], center[1], center[2] - half[2]], [half[0], half[1], size[2] * 0.06]);
  const open = smoothUnion(1.4, [dome, floor]);
  const pillar = capsule([center[0], center[1], center[2] - half[2]], [center[0], center[1], center[2] + half[2] * 0.7], size[0] * 0.1);
  const carved = subtract(open, pillar); // pillar region becomes solid rock
  return displace(carved, (params.noise ?? 0.25) * 0.6, 0.12, seed);
}

export const demoHullArchetypes: HullArchetypeRegistry = {
  // biome-flavoured general rooms
  nave: { id: "nave", archetype: "hall", sizeRange: { min: [10, 6, 5], max: [24, 12, 9] }, noise: 0.18, biomes: ["gothic-flooded"], weight: 1 },
  "flooded-cavern": { id: "flooded-cavern", archetype: "cavern", sizeRange: { min: [12, 12, 6], max: [28, 28, 13] }, noise: 0.55, roomKinds: ["cavern", "indoor", "arena"], biomes: ["gothic-flooded"], weight: 0.8 },
  reliquary: { id: "reliquary", archetype: "rotunda", sizeRange: { min: [10, 10, 6], max: [20, 20, 12] }, noise: 0.2, biomes: ["sci-crypt"], weight: 0.8 },
  liftshaft: { id: "liftshaft", archetype: "shaft", sizeRange: { min: [8, 8, 12], max: [14, 14, 30] }, noise: 0.22, roomKinds: ["shaft", "vertical"], weight: 0.45 },
  "open-terrace": { id: "open-terrace", archetype: "outdoor-open", sizeRange: { min: [22, 22, 10], max: [40, 40, 18] }, noise: 0.42, outdoor: true, weight: 0.35 },
  // the authored landmark set-piece (Area places 1–2 with sightlines)
  "grand-atrium": { id: "grand-atrium", sdf: grandAtrium, sizeRange: { min: [22, 22, 16], max: [34, 34, 26] }, noise: 0.3, roomKinds: ["hub", "boss-chamber"], landmark: true, weight: 0.5 },
};

const gothicFlooded: BiomePack = {
  id: "gothic-flooded",
  palette: ["#141a24", "#243244", "#39516b", "#5d7fa0", "#9fc0d8"],
  materials: { floor: "wet-stone", wall: "gothic-brick", ceiling: "vaulted-stone" },
  noise: { amp: 0.4, freq: 0.16 },
  dressing: ["water", "stalactite", "foliage", "rubble"],
  subBiomes: { crypt: 0.6, cistern: 0.4 },
};

const sciCrypt: BiomePack = {
  id: "sci-crypt",
  palette: ["#1c1a20", "#332f3d", "#4f4a5e", "#7d7796", "#c0bcd0"],
  materials: { floor: "ashen-plate", wall: "riveted-steel", ceiling: "conduit-panel" },
  noise: { amp: 0.25, freq: 0.14 },
  dressing: ["rubble", "stalactite"],
  subBiomes: { reactor: 0.5, ossuary: 0.5 },
};

export const demoBiomes: BiomeRegistry = {
  "gothic-flooded": gothicFlooded,
  "sci-crypt": sciCrypt,
};
