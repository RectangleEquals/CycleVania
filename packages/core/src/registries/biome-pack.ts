/**
 * Biome pack — a biome is a rich CONTENT pack, not just a palette: a colour ramp,
 * material ids, surface-noise params, dressing sets, a poly budget, and sub-biome
 * weights for gradient zoning. The game supplies these; the geometry/dressing passes
 * consume them. All data (no meshes).
 */

export interface BiomePack {
  id: string;
  /** Ordered palette ramp (hex ints or css strings) — game maps to materials. */
  palette: string[];
  /** Material ids per surface kind (floor/wall/ceiling/…). */
  materials?: Record<string, string>;
  /** Surface-noise controls for the hull displacement (amplitude/frequency). */
  noise?: { amp: number; freq: number };
  /** Dressing props allowed (water/stalactite/foliage/rubble/…). */
  dressing?: string[];
  /** Per-tier poly budgets (parent realizer honours; CycleVania stays poly-agnostic). */
  polyBudget?: { env?: number; prop?: number };
  /** Sub-biomes blended across an Area with seam gradients → weights. */
  subBiomes?: Record<string, number>;
}

export type BiomeRegistry = Record<string, BiomePack>;

export const DEFAULT_BIOME: BiomePack = {
  id: "default",
  palette: ["#2a2f3a", "#3d4757", "#5b6b82", "#8aa0b8", "#c3d2e0"],
  noise: { amp: 0.3, freq: 0.15 },
  dressing: ["water", "stalactite", "foliage", "rubble"],
};
