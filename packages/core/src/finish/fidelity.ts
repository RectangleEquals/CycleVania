/**
 * Fidelity profile — the spectrum knob. `angleStepDeg`: 90 (orthogonal dungeon),
 * 45, 15, 5 (PS2 faceting), or null (smooth). Together with hull/noise choices,
 * this config IS the fidelity spectrum. `quantizeNormal` lives in math; this just
 * carries the profile + a bound helper.
 */

import { quantizeNormal, type Vec3 } from "../math/index.js";

export interface FidelityProfile {
  angleStepDeg: number | null;
  voxelRes: number;
  maxDim: number;
  snapNormals: boolean;
}

export const DEFAULT_FIDELITY: FidelityProfile = { angleStepDeg: 5, voxelRes: 1, maxDim: 72, snapNormals: true };

export function snapNormal(n: Vec3, profile: FidelityProfile): Vec3 {
  return profile.snapNormals ? quantizeNormal(n, profile.angleStepDeg) : quantizeNormal(n, null);
}
