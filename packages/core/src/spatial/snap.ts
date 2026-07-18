/**
 * Snap policy — constrains geometry angles. `ps2` snaps to the retro 45° palette
 * (the default look we WANT: 45° cuts, chamfers, curves, not pure 90° boxes);
 * `free` passes angles through for Metroid-Prime-granularity geometry.
 */

import type { SnapPolicy } from "../types.js";

const PS2_STEP = Math.PI / 4; // 45°

/** Snap a yaw (radians) to the policy's angle palette. */
export function snapAngle(rad: number, policy: SnapPolicy): number {
  if (policy === "free") return rad;
  return Math.round(rad / PS2_STEP) * PS2_STEP;
}

/** The allowed yaw palette (radians) for a policy — `null` means continuous. */
export function anglePalette(policy: SnapPolicy): number[] | null {
  if (policy === "free") return null;
  return [0, 1, 2, 3, 4, 5, 6, 7].map((i) => i * PS2_STEP);
}
