/**
 * Fidelity config — the PS2 "intent without laziness" look, as data. Angle grid is 5°
 * (flats/chamfers tend to land on 15°, radial shapes step as fine as 5°); voxel/grid
 * resolution controls how finely the dual-contour pass samples the field.
 */

export interface FidelityConfig {
  /** Normal-quantization step in degrees (default 5). */
  angleStepDeg: number;
  /** Dual-contouring sample resolution in world units (smaller = finer facets). */
  voxelRes: number;
  /** Snap generated normals to the angle grid (the PS2 facet look). */
  snapNormals: boolean;
}

export const DEFAULT_FIDELITY: FidelityConfig = {
  angleStepDeg: 5,
  voxelRes: 1,
  snapNormals: true,
};
