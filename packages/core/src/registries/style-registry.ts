/**
 * Style registry — texture/palette-agnostic style descriptors. CycleVania only
 * uses the opaque `id` (carried into descriptors) and room-archetype weights; the
 * parent maps `id` → its own textures/materials/palette.
 */
export interface StyleDef {
  id: string;
  /** Weight per room-archetype id when this style is active. */
  roomWeights?: Record<string, number>;
  /** Opaque biome tag the parent maps to a palette. */
  biome?: string;
  tags?: string[];
}
