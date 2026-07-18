/**
 * Lock-and-key vocabulary as data. Each named lock is a `ChallengeTemplate`: the
 * `solvedBy` rule (identical to what the solver gates on) plus the spatial
 * `recipe` archetype that physically expresses it. A few show side-effects
 * (shatter/rappel opening a drop → a new edge) and compound/counted gates.
 */

import type { LockVocabularyInput } from "@cyclevania/core";

export const lockCatalog: LockVocabularyInput = {
  "anchor-gap": { solvedBy: (r) => r.have("grapple"), recipe: "gap-crossing" },
  "lethal-drop": { solvedBy: (r) => r.have("rappel"), recipe: "reversible-drop", sideEffects: { addsEdge: { traversal: "drop", oneWay: false } } },
  "high-ledge": { solvedBy: (r) => r.have("leap"), recipe: "high-ledge" },
  "thin-lattice": { solvedBy: (r) => r.have("phase"), recipe: "phase-wall" },
  "ceiling-route": { solvedBy: (r) => r.have("invert"), recipe: "gravity-field" },
  "ferric-rail": { solvedBy: (r) => r.have("magnet"), recipe: "ferric-traversal" },
  "weight-plate": { solvedBy: (r) => r.have("anchor"), recipe: "weight-plate" },
  torrent: { solvedBy: (r) => r.have("freeze"), recipe: "freeze-crossing" },
  "flood-basin": { solvedBy: (r) => r.have("siphon"), recipe: "fluid-level" },
  "fertile-shaft": { solvedBy: (r) => r.have("cultivate"), recipe: "grow-bridge" },
  "cold-brazier": { solvedBy: (r) => r.have("ignite"), recipe: "ignite-gate" },
  "brittle-seal": { solvedBy: (r) => r.have("shatter"), recipe: "sealed-barrier", sideEffects: { addsEdge: { traversal: "drop", oneWay: true } } },
  "dead-machine": { solvedBy: (r) => r.have("charge"), recipe: "powered-mechanism" },
  "hidden-crossing": { solvedBy: (r) => r.have("reveal"), recipe: "hidden-crossing" },
  "glyph-door": { solvedBy: (r) => r.have("translate"), recipe: "cipher-gate" },
  "hazard-field": { solvedBy: (r) => r.have("ward"), recipe: "hazard-field" },
  "moving-crusher": { solvedBy: (r) => r.have("stasis"), recipe: "moving-platform" },
  "harmonic-seal": { solvedBy: (r) => r.have("resonate"), recipe: "sound-gate" },
  "decoy-gate": { solvedBy: (r) => r.have("lure"), recipe: "decoy-gate" },
  crawlspace: { solvedBy: (r) => r.have("small-form"), recipe: "crawlspace" },
  "beam-shield": { solvedBy: (r) => r.have("reflect"), recipe: "beam-redirect" },
  // counted + compound examples
  "reliquary-seal": { solvedBy: (r) => r.count("shard-cache", 3), recipe: "multi-key" },
  "deep-vault": { solvedBy: (r) => r.and(r.have("grapple"), r.have("reveal")), recipe: "compound" },
};
