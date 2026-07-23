/**
 * The finish pass (L4) — convert an Area's composed field into engine-agnostic
 * geometry: a deduplicated kit + instances, an occupancy grid, dressing anchors.
 * Strictly a finishing pass: converts, classifies, exports — no structural
 * decisions. `finishArea` is the entry; `composeReachFinish` drives it per Area.
 */

import type { WorldBox } from "../math/index.js";
import { fnv1a } from "../math/index.js";
import type { AreaField } from "../volume/field.js";
import type { ReachSkeleton } from "../skeleton/space-plan.js";
import { DEFAULT_FIDELITY, type FidelityProfile } from "./fidelity.js";
import { dualContour, dualContourSteps, type Mesh } from "./mesher.js";
import { meshToKit, type GeneratedKit, type PieceInstance } from "./kit.js";
import { occupancyGrid, toOccupancyData, type OccupancyGrid, type OccupancyData } from "./occupancy.js";
import { dressArea, type DressingAnchor } from "./dress.js";

export * from "./fidelity.js";
export { dualContour, dualContourSteps } from "./mesher.js";
export type { Mesh } from "./mesher.js";
export { meshToKit } from "./kit.js";
export type { GeneratedKit, GeneratedPiece, PieceInstance, PieceMeta, KitOptions } from "./kit.js";
export { occupancyGrid, cellOf, isSolidAt, isWalkable, collideSphere, toOccupancyData } from "./occupancy.js";
export type { OccupancyGrid, OccupancyData } from "./occupancy.js";
export { dressArea } from "./dress.js";
export type { DressingAnchor } from "./dress.js";

export interface FinishBudgets {
  polyBudgetPerArea?: number;
  maxUniquePieces?: number;
  cellSizeMultiple?: number; // kit cell = res × this (default 4)
}

export interface FinishResult {
  kit: GeneratedKit;
  instances: PieceInstance[];
  occupancy: OccupancyGrid;
  occupancyData: OccupancyData;
  dressing: DressingAnchor[];
  stats: { tris: number; uniquePieces: number; res: number };
}

export interface FinishAreaOptions {
  biome: string;
  fidelity?: FidelityProfile;
  revealableBoxes?: WorldBox[];
  budgets?: FinishBudgets;
  dressDensity?: number;
  seed: number;
}

export function finishArea(field: AreaField, opts: FinishAreaOptions): FinishResult {
  const fidelity = opts.fidelity ?? DEFAULT_FIDELITY;
  const ext = field.extents;
  const mesh: Mesh = dualContour(field.sdf, ext, fidelity);
  const cellSize = ext.res * (opts.budgets?.cellSizeMultiple ?? 4);

  const { kit, instances } = meshToKit(mesh, ext.origin, cellSize, {
    biome: opts.biome,
    ...(opts.budgets?.polyBudgetPerArea !== undefined ? { polyBudget: opts.budgets.polyBudgetPerArea } : {}),
    ...(opts.budgets?.maxUniquePieces !== undefined ? { maxUniquePieces: opts.budgets.maxUniquePieces } : {}),
    ...(opts.revealableBoxes ? { revealableBoxes: opts.revealableBoxes } : {}),
  });

  const occupancy = occupancyGrid(field.sdf, ext.origin, ext.dims, ext.res);
  const dressing = dressArea(occupancy, opts.seed, opts.dressDensity ?? 0.35);

  return {
    kit,
    instances,
    occupancy,
    occupancyData: toOccupancyData(occupancy),
    dressing,
    stats: { tris: mesh.indices.length / 3, uniquePieces: kit.pieces.length, res: ext.res },
  };
}

export interface ComposeReachFinishOptions {
  seed: string;
  fidelity?: FidelityProfile;
  budgets?: FinishBudgets;
}

/** Run the finish pass over every Area that has a composed field. */
export function composeReachFinish(skeleton: ReachSkeleton, opts: ComposeReachFinishOptions): void {
  for (const area of skeleton.areas) {
    if (!area.field) continue;
    const revealableBoxes = area.spaces.filter((s) => s.hidden).map((s) => s.envelope);
    const biome = area.spaces[0]?.biome ?? "default";
    area.finish = finishArea(area.field, {
      biome,
      ...(opts.fidelity ? { fidelity: opts.fidelity } : {}),
      ...(opts.budgets ? { budgets: opts.budgets } : {}),
      ...(revealableBoxes.length > 0 ? { revealableBoxes } : {}),
      seed: fnv1a(`${opts.seed}:${area.regionId}`),
    });
  }
}
