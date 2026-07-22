/**
 * Item catalog — the game's items as data. Progression items grant a capability
 * (the solver gates on these). Each capability can carry a `CapabilityProfile`
 * telling the generator what it affords the space budget (the affordance half of
 * the affordance/challenge contract).
 */

import type { Capability } from "../logic/index.js";
import type { ItemClass } from "../fill/fill-policy.js";

export type { ItemClass };

/** What a capability affords the GENERATOR's space budget (open axis set). */
export interface CapabilityProfileGrants {
  reachUp?: number;
  gapSpan?: number;
  descend?: "safe" | "reverse";
  throughMatter?: boolean;
  revealHidden?: boolean;
  hazardImmune?: string[];
  massDelta?: number;
  timeControl?: boolean;
  energyRoute?: boolean;
  [axis: string]: number | string | boolean | string[] | undefined;
}

/** Generation biases applied when a capability is IN SCOPE for the party. */
export interface CapabilityProfileBias {
  zWeight?: number;
  loopWeight?: number;
  hazardWeight?: number;
  enableTags?: string[];
}

export interface CapabilityProfile {
  grants?: CapabilityProfileGrants;
  bias?: CapabilityProfileBias;
  /**
   * Relative player-power of this capability (≈0 lateral/minor … 1 world-reshaping).
   * Drives the gadget economy's weighted, depth-scheduled draw: early reaches favour
   * low-power/lateral gadgets, later reaches allow higher-power/vertical ones. When
   * omitted, a default is derived from `bias.zWeight` + grant magnitude.
   */
  power?: number;
}

/** What `/use <item>` does in the simulator (data-driven). */
export interface UseEffect {
  grants?: Capability;
  charges?: number;
  setsFlag?: string;
  consumes?: boolean;
  /** Names a lock/recipe this use satisfies (e.g. lantern → "hidden"). */
  reveals?: string;
}

export interface ItemDef {
  id: string;
  class: ItemClass;
  /** Progression items MUST declare the capability they grant. */
  grants?: Capability;
  name?: string;
  use?: UseEffect;
  profile?: CapabilityProfile;
}

export interface ItemCatalogInput {
  catalog: ItemDef[];
  startCaps?: Capability[];
}
