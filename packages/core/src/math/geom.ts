/**
 * Axis-aligned box helpers used by cell-occupancy planning and area bounds.
 */

import type { Vec3, WorldBox } from "./vec.js";

export const boxFromCenterHalf = (center: Vec3, half: Vec3): WorldBox => ({
  min: [center[0] - half[0], center[1] - half[1], center[2] - half[2]],
  max: [center[0] + half[0], center[1] + half[1], center[2] + half[2]],
});

export const boxCenter = (b: WorldBox): Vec3 => [
  (b.min[0] + b.max[0]) / 2,
  (b.min[1] + b.max[1]) / 2,
  (b.min[2] + b.max[2]) / 2,
];

export const boxSize = (b: WorldBox): Vec3 => [
  b.max[0] - b.min[0],
  b.max[1] - b.min[1],
  b.max[2] - b.min[2],
];

/** Do two boxes overlap? A shared face (touching) counts as overlap only within `margin`. */
export function boxOverlap(a: WorldBox, b: WorldBox, margin = 0): boolean {
  return (
    a.min[0] - margin < b.max[0] &&
    a.max[0] + margin > b.min[0] &&
    a.min[1] - margin < b.max[1] &&
    a.max[1] + margin > b.min[1] &&
    a.min[2] - margin < b.max[2] &&
    a.max[2] + margin > b.min[2]
  );
}

export function boxContainsPoint(b: WorldBox, p: Vec3): boolean {
  return (
    p[0] >= b.min[0] &&
    p[0] <= b.max[0] &&
    p[1] >= b.min[1] &&
    p[1] <= b.max[1] &&
    p[2] >= b.min[2] &&
    p[2] <= b.max[2]
  );
}

/** Smallest box containing both inputs. */
export function boxUnion(a: WorldBox, b: WorldBox): WorldBox {
  return {
    min: [Math.min(a.min[0], b.min[0]), Math.min(a.min[1], b.min[1]), Math.min(a.min[2], b.min[2])],
    max: [Math.max(a.max[0], b.max[0]), Math.max(a.max[1], b.max[1]), Math.max(a.max[2], b.max[2])],
  };
}

/** Union of many boxes (throws on empty — callers always have ≥1). */
export function boxUnionAll(boxes: readonly WorldBox[]): WorldBox {
  const first = boxes[0];
  if (!first) throw new Error("boxUnionAll: empty");
  let acc = first;
  for (let i = 1; i < boxes.length; i++) acc = boxUnion(acc, boxes[i] as WorldBox);
  return acc;
}
