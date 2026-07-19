/**
 * Snap policy — constrains geometry angles. `ps2` snaps to a **5° grid** (the
 * minimum increment; flats/chamfers tend to 15°, radial/curved shapes step as
 * fine as 5°), which is the retro-but-not-blocky look we want — Bézier/spline
 * curves are welcome, just quantized to 5°. `free` passes angles through for
 * arbitrary geometry.
 */

import type { SnapPolicy } from "../types.js";

const PS2_STEP = Math.PI / 36; // 5°

/** Snap a yaw (radians) to the policy's angle grid (5° for ps2). */
export function snapAngle(rad: number, policy: SnapPolicy): number {
  if (policy === "free") return rad;
  return Math.round(rad / PS2_STEP) * PS2_STEP;
}

/** The allowed yaw palette (radians) for a policy — `null` means continuous. */
export function anglePalette(policy: SnapPolicy): number[] | null {
  if (policy === "free") return null;
  return Array.from({ length: 72 }, (_, i) => i * PS2_STEP); // 0,5,10,…,355°
}
