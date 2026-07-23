export { BUILTIN_BUCKETS } from "./capability-def.js";
export type {
  BuiltinBucket,
  Bucket,
  FacetContext,
  MagnitudeFacet,
  TagFacet,
  ResourceFacet,
  Facet,
  CapabilityDef,
  GadgetDef,
} from "./capability-def.js";
export { buildHeld, aggregateBuckets, activeTags } from "./facets.js";
export { DEFAULT_GADGET_ECONOMY } from "./economy.js";
export type { GadgetEconomyConfig } from "./economy.js";
export { gadgetEntry } from "./gadget-scheduler.js";
