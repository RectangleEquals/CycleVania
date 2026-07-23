/**
 * Content anchors — the one placement primitive for items, interactables, lights,
 * props, dressing, landmarks, vistas, refill sites. A `ContentAnchor` binds
 * gameplay content to a real point on real surface; its `binding` (if any) tells
 * the host what to spawn.
 */

import type { Vec3 } from "../math/index.js";
import type { SurfaceKind } from "../types.js";

export type AnchorBinding =
  | { type: "location"; locationId: string; itemId?: string; sphere?: number }
  | { type: "puzzle"; puzzleInstanceId: string }
  | { type: "refill"; poolId: string }
  | { type: "landmark" }
  | { type: "vista"; landmarkSpaceId: string };

export interface ContentAnchor {
  id: string;
  spaceId: string;
  kindId: string;
  pos: Vec3;
  up: Vec3;
  surface: SurfaceKind;
  binding?: AnchorBinding;
  tags: string[];
}

export interface ContentAnchorKind {
  id: string;
  allowedSurfaces: SurfaceKind[];
  minSeparation: number;
  clearanceFromStructural: number;
  targetDensity: number; // absolute anchors per Space for this kind
}

export const DEFAULT_ANCHOR_KINDS: Record<string, ContentAnchorKind> = {
  "gadget-pickup": { id: "gadget-pickup", allowedSurfaces: ["floor", "slope"], minSeparation: 3, clearanceFromStructural: 2, targetDensity: 2 },
  interactable: { id: "interactable", allowedSurfaces: ["floor", "wall", "slope"], minSeparation: 3, clearanceFromStructural: 2, targetDensity: 3 },
  light: { id: "light", allowedSurfaces: ["ceiling", "wall", "overhang"], minSeparation: 4, clearanceFromStructural: 1, targetDensity: 3 },
  prop: { id: "prop", allowedSurfaces: ["floor", "slope"], minSeparation: 2, clearanceFromStructural: 1.5, targetDensity: 4 },
  dressing: { id: "dressing", allowedSurfaces: ["floor", "wall", "ceiling", "slope", "overhang"], minSeparation: 1.5, clearanceFromStructural: 1, targetDensity: 6 },
  "landmark-feature": { id: "landmark-feature", allowedSurfaces: ["floor", "slope"], minSeparation: 8, clearanceFromStructural: 3, targetDensity: 1 },
  vista: { id: "vista", allowedSurfaces: ["floor", "slope"], minSeparation: 6, clearanceFromStructural: 2, targetDensity: 1 },
  "refill-site": { id: "refill-site", allowedSurfaces: ["floor", "slope"], minSeparation: 5, clearanceFromStructural: 2, targetDensity: 1 },
};
