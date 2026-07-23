/**
 * The example 24-verb gadget catalog — one distinct mechanical verb each, diegetic
 * naming (no gear-slot nouns). Consumed by CycleVania as registry data. Each gadget
 * grants one capability whose Facets tell the generator its consequences (Magnitude
 * → a budget bucket; Tag → tagged geometry/puzzles). Purely illustrative — a host
 * supplies its own catalog.
 */

import type { Bucket, CapabilityDef, Facet, GadgetDef } from "@cyclevania/core";

const tag = (t: string): Facet => ({ kind: "tag", tag: t });
const mag = (bucket: Bucket, v: number): Facet => ({ kind: "magnitude", bucket, evaluate: () => v });
const cap = (id: string, power: number, facets: Facet[], category: string): CapabilityDef => ({ id, held: "granted", facets, powerWeight: () => power, category });

// Movement / traversal verbs
export const CAPABILITIES: CapabilityDef[] = [
  cap("grapple", 0.4, [tag("grapple-point"), mag("traversal.xyGap", 6)], "movement"),
  cap("rappel", 0.45, [tag("rappel-anchor"), mag("traversal.zDown", 8)], "movement"),
  cap("leap", 0.7, [mag("traversal.zUp", 4)], "movement"),
  cap("phase", 0.75, [tag("permeable-wall"), mag("traversal.permeate", 1)], "movement"),
  cap("recall", 0.72, [tag("recall-beacon")], "movement"),
  cap("invert", 0.85, [tag("gravity-field"), mag("traversal.zUp", 6)], "movement"),
  cap("magnet", 0.35, [tag("ferric-rail")], "movement"),
  cap("anchor", 0.4, [tag("weight-plate"), mag("traversal.weight", 3)], "world"),
  cap("small-form", 0.3, [tag("crawl-aperture")], "movement"),
  // World / environment verbs
  cap("freeze", 0.5, [tag("solidify-fluid")], "world"),
  cap("siphon", 0.55, [tag("liquid-level")], "world"),
  cap("cultivate", 0.5, [tag("fertile-anchor")], "world"),
  cap("ignite", 0.5, [tag("combustible")], "world"),
  cap("shatter", 0.6, [tag("brittle-wall"), mag("traversal.zDown", 4)], "world"),
  cap("charge", 0.6, [tag("dormant-machine")], "world"),
  cap("reveal", 0.68, [tag("revealable"), mag("traversal.perceive", 2)], "world"),
  cap("translate", 0.2, [tag("glyph-lock")], "world"),
  cap("ward", 0.55, [tag("hazard-ward"), mag("traversal.timeHazard", 3)], "world"),
  cap("stasis", 0.65, [tag("moving-hazard")], "world"),
  cap("resonate", 0.6, [tag("harmonic-seal")], "world"),
  cap("lure", 0.4, [tag("sound-gate")], "world"),
  cap("reflect", 0.7, [tag("beam-receiver")], "world"),
  // Boss verbs (gate only behind earlier-sphere items)
  cap("sunder-core", 0.9, [tag("boss-shell"), mag("challenge.offense", 0.7)], "combat"),
  cap("sever", 0.9, [tag("boss-tether"), mag("challenge.offense", 0.6)], "combat"),
];

export const GADGETS: GadgetDef[] = CAPABILITIES.map((c) => ({ id: `the-${c.id}`, grants: [c.id] }));

export const GADGET_CATALOG: { capabilities: CapabilityDef[]; gadgets: GadgetDef[] } = { capabilities: CAPABILITIES, gadgets: GADGETS };
