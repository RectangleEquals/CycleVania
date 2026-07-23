/**
 * Spatial recipes — where a logical lock becomes physical space. A `PuzzleDef`'s
 * `spatialRecipe` names a `SpatialRecipeDef` L2 reserves an envelope for and L3
 * realizes. Shipped as data (overridable). `Vec3` extent is [x,y,z] world-grid
 * cells.
 */

import type { NodeRole } from "../graph/index.js";
import type { Traversal } from "../types.js";

export interface SpatialRecipeDef {
  id: string;
  space: {
    kinds: ("room" | "outdoor" | "connector")[];
    minExtent: [number, number, number];
    preferRoles?: NodeRole[];
    biomeAffinity?: string[];
  };
  sockets?: { count: number; traversal: Traversal; signature: string }[];
  anchors: { kindId: string; count: { min: number; max: number }; tags?: string[] }[];
  geometry?: { carve?: string; revealable?: boolean; scaleWith?: "depth" | "budget" };
  difficulty: { base: number; scaleWithDepth?: number };
}

const R = (
  id: string,
  kinds: ("room" | "outdoor" | "connector")[],
  minExtent: [number, number, number],
  anchors: { kindId: string; count: { min: number; max: number }; tags?: string[] }[],
  extra: Partial<SpatialRecipeDef> = {},
): SpatialRecipeDef => ({ id, space: { kinds, minExtent }, anchors, difficulty: { base: 0.4, scaleWithDepth: 0.1 }, ...extra });

/** The 14 shipped recipe archetypes (modest defaults; presets refine them). */
export const SHIPPED_RECIPES: SpatialRecipeDef[] = [
  R("gap-crossing", ["room", "connector"], [8, 4, 3], [{ kindId: "interactable", count: { min: 0, max: 0 } }], { geometry: { carve: "hazard-trench", scaleWith: "depth" } }),
  R("high-ledge", ["room"], [6, 6, 5], [{ kindId: "interactable", count: { min: 0, max: 1 } }], { geometry: { carve: "ledge-shelf", scaleWith: "depth" } }),
  R("hidden-crossing", ["room"], [8, 4, 3], [{ kindId: "interactable", count: { min: 0, max: 1 } }], { geometry: { carve: "hazard-trench", revealable: true } }),
  R("hazard-field", ["room", "outdoor"], [8, 8, 3], [{ kindId: "prop", count: { min: 2, max: 6 } }], { geometry: { carve: "hazard-trench" } }),
  R("moving-route", ["room"], [8, 6, 5], [{ kindId: "interactable", count: { min: 1, max: 2 } }]),
  R("sealed-barrier", ["room", "connector"], [4, 4, 4], [{ kindId: "interactable", count: { min: 0, max: 0 } }], { geometry: { carve: "sealed-wall" } }),
  R("powered-mechanism", ["room"], [6, 6, 4], [{ kindId: "interactable", count: { min: 1, max: 3 }, tags: ["power-node"] }]),
  R("panel-array", ["room"], [8, 8, 4], [{ kindId: "interactable", count: { min: 2, max: 5 }, tags: ["switch-panel"] }]),
  R("sequence-path", ["room"], [10, 10, 4], [{ kindId: "interactable", count: { min: 3, max: 5 }, tags: ["ordered-switch"] }]),
  R("combination-lock", ["room"], [6, 6, 4], [{ kindId: "interactable", count: { min: 3, max: 5 }, tags: ["dial"] }]),
  R("weight-plate", ["room"], [6, 6, 4], [{ kindId: "interactable", count: { min: 1, max: 3 }, tags: ["pressure-plate"] }]),
  R("arena", ["room", "outdoor"], [10, 10, 5], [{ kindId: "interactable", count: { min: 0, max: 0 } }], { space: { kinds: ["room", "outdoor"], minExtent: [10, 10, 5], preferRoles: ["capstone", "gate"] }, difficulty: { base: 0.6, scaleWithDepth: 0.1 } }),
  R("collectathon-shrine", ["room"], [8, 8, 6], [{ kindId: "landmark-feature", count: { min: 1, max: 1 } }], { difficulty: { base: 0.9 } }),
  R("boss-arena", ["room", "outdoor"], [14, 14, 8], [{ kindId: "landmark-feature", count: { min: 1, max: 1 } }], { space: { kinds: ["room", "outdoor"], minExtent: [14, 14, 8], preferRoles: ["capstone"] }, difficulty: { base: 0.9, scaleWithDepth: 0.05 } }),
];

export const SHIPPED_RECIPES_BY_ID: ReadonlyMap<string, SpatialRecipeDef> = new Map(SHIPPED_RECIPES.map((r) => [r.id, r]));
