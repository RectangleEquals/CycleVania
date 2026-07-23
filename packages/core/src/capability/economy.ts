/** Per-Reach progression-item count range. */

export interface GadgetEconomyConfig {
  min: number;
  max: number;
}

export const DEFAULT_GADGET_ECONOMY: GadgetEconomyConfig = { min: 1, max: 3 };
