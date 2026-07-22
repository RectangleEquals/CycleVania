/**
 * Hull-archetype registry — the HYBRID room-volume grammar. A game supplies (as data)
 * a set of parametric SDF room shapes drawn deterministically by biome + size + budget,
 * plus optional authored landmark set-piece SDFs for memorability. CycleVania samples
 * these into the area field (volume/field.ts) instead of painting hollow boxes.
 */

import type { Sdf } from "../volume/index.js";
import type { HullParams } from "../volume/hulls.js";

export interface HullArchetypeDef {
  id: string;
  /** Name in the built-in `HULL_ARCHETYPES` (hall/rotunda/cavern/shaft/outdoor-open) or a custom sdf. */
  archetype?: string;
  /** A game-authored landmark/set-piece SDF (overrides `archetype`) — the HYBRID escape hatch. */
  sdf?: (p: HullParams) => Sdf;
  /** XYZ size envelope [min,max] in world units. */
  sizeRange: { min: [number, number, number]; max: [number, number, number] };
  /** Surface noise amplitude (organic roughness). */
  noise?: number;
  /** Which room kinds/biomes this shape suits (empty = any). */
  roomKinds?: string[];
  biomes?: string[];
  /** Open-top sky volume (outdoor). */
  outdoor?: boolean;
  /** A memorable navigation anchor (Area places 1–2 with sightlines). */
  landmark?: boolean;
  /** Selection weight (budget-scaled for large/outdoor archetypes). */
  weight?: number;
}

export type HullArchetypeRegistry = Record<string, HullArchetypeDef>;

/** Built-in general archetypes so the generator works with no game-supplied hulls. */
export const DEFAULT_HULL_ARCHETYPES: HullArchetypeRegistry = {
  hall: { id: "hall", archetype: "hall", sizeRange: { min: [10, 6, 5], max: [22, 12, 8] }, noise: 0.15, weight: 1 },
  rotunda: { id: "rotunda", archetype: "rotunda", sizeRange: { min: [10, 10, 6], max: [20, 20, 12] }, noise: 0.2, weight: 0.8 },
  cavern: { id: "cavern", archetype: "cavern", sizeRange: { min: [12, 12, 7], max: [26, 26, 14] }, noise: 0.5, roomKinds: ["cavern", "arena"], weight: 0.7 },
  shaft: { id: "shaft", archetype: "shaft", sizeRange: { min: [8, 8, 12], max: [14, 14, 28] }, noise: 0.25, roomKinds: ["shaft", "vertical"], weight: 0.4 },
  "outdoor-open": { id: "outdoor-open", archetype: "outdoor-open", sizeRange: { min: [20, 20, 10], max: [40, 40, 18] }, noise: 0.4, outdoor: true, weight: 0.3 },
};
