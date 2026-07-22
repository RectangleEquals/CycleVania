/**
 * The 24-gadget "Starwrought Instrument" catalog — one distinct mechanical verb
 * each — expressed as CycleVania `ItemDef` data. This is EXAMPLE/game content fed
 * INTO the engine, not part of the engine. Each entry declares the capability it
 * grants (what the solver gates on), a `use` effect (what the simulator does), and
 * a `profile` (what it affords the generator's space budget — the affordance half
 * of the affordance/challenge contract).
 *
 * Names avoid gear-slot nouns so they never read as equippable loot.
 */

import type { ItemDef } from "@cyclevania/core";

export const gadgetCatalog: ItemDef[] = [
  {
    id: "ardent-skyhook",
    name: "The Ardent Skyhook",
    class: "progression",
    grants: "grapple",
    use: { grants: "grapple", charges: 3 },
    profile: { grants: { gapSpan: 4, reachUp: 3 }, bias: { loopWeight: 0.2, enableTags: ["anchor"] }, power: 0.4 },
  },
  {
    id: "gravemoth-filament",
    name: "Gravemoth Filament",
    class: "progression",
    grants: "rappel",
    use: { grants: "rappel", charges: 3 },
    profile: { grants: { descend: "reverse" }, bias: { loopWeight: 0.35, enableTags: ["drop"] } },
  },
  {
    id: "second-wind-censer",
    name: "Censer of the Second Wind",
    class: "progression",
    grants: "leap",
    use: { grants: "leap", charges: 2 },
    // double-jump: makes the WHOLE world more vertical → high power, unlikely (not impossible) early
    profile: { grants: { reachUp: 2, gapSpan: 2 }, bias: { zWeight: 0.4 }, power: 0.7 },
  },
  {
    id: "wend-stone",
    name: "The Wend-Stone",
    class: "progression",
    grants: "phase",
    use: { grants: "phase", charges: 2 },
    profile: { grants: { throughMatter: true }, bias: { enableTags: ["lattice"] }, power: 0.75 },
  },
  {
    id: "waystone-beacon",
    name: "The Waystone Beacon",
    class: "progression",
    grants: "recall",
    use: { grants: "recall", charges: 1 },
    profile: { bias: { loopWeight: 0.5 }, power: 0.72 },
  },
  {
    id: "antipode-governor",
    name: "The Antipode Governor",
    class: "progression",
    grants: "invert",
    use: { grants: "invert", charges: 2 },
    // flip gravity: the most world-reshaping traversal verb → highest power
    profile: { grants: { massDelta: -1 }, bias: { zWeight: 0.5, enableTags: ["ceiling"] }, power: 0.85 },
  },
  {
    id: "lodestar-coil",
    name: "The Lodestar Coil",
    class: "progression",
    grants: "magnet",
    use: { grants: "magnet", charges: 3 },
    profile: { grants: { gapSpan: 3 }, bias: { enableTags: ["ferric"] }, power: 0.35 },
  },
  {
    id: "anchorites-burden",
    name: "The Anchorite's Burden",
    class: "progression",
    grants: "anchor",
    use: { grants: "anchor", charges: 2 },
    profile: { grants: { massDelta: 2 }, bias: { enableTags: ["weight-plate", "current"] } },
  },
  {
    id: "stillwater-diapason",
    name: "The Stillwater Diapason",
    class: "progression",
    grants: "freeze",
    use: { grants: "freeze", charges: 3 },
    profile: { grants: { gapSpan: 2 }, bias: { enableTags: ["liquid"] } },
  },
  {
    id: "tidewright-cistern",
    name: "The Tidewright Cistern",
    class: "progression",
    grants: "siphon",
    use: { grants: "siphon", charges: 2 },
    profile: { bias: { enableTags: ["liquid", "basin"] } },
  },
  {
    id: "quickening-bough",
    name: "The Quickening Bough",
    class: "progression",
    grants: "cultivate",
    use: { grants: "cultivate", charges: 2 },
    profile: { grants: { reachUp: 2 }, bias: { zWeight: 0.3, enableTags: ["fertile"] } },
  },
  {
    id: "everbrand-ferula",
    name: "The Everbrand Ferula",
    class: "progression",
    grants: "ignite",
    use: { grants: "ignite", charges: 4 },
    profile: { grants: { hazardImmune: ["cold"] }, bias: { hazardWeight: 0.2, enableTags: ["growth", "brazier"] } },
  },
  {
    id: "sundering-charge",
    name: "The Sundering Charge",
    class: "progression",
    grants: "shatter",
    use: { grants: "shatter", charges: 3 },
    profile: { bias: { loopWeight: 0.25, enableTags: ["brittle", "sealed"] } },
  },
  {
    id: "voltaic-reliquary",
    name: "The Voltaic Reliquary",
    class: "progression",
    grants: "charge",
    use: { grants: "charge", charges: 3 },
    profile: { grants: { energyRoute: true }, bias: { enableTags: ["machine"] } },
  },
  {
    id: "wan-lantern",
    name: "The Wan Lantern",
    class: "progression",
    grants: "reveal",
    use: { grants: "reveal", reveals: "hidden", charges: 4 },
    profile: { grants: { revealHidden: true }, bias: { enableTags: ["hidden", "ghost-platform"] }, power: 0.68 },
  },
  {
    id: "glossolith",
    name: "The Glossolith",
    class: "progression",
    grants: "translate",
    use: { grants: "translate" },
    profile: { bias: { enableTags: ["glyph", "cipher"] }, power: 0.2 },
  },
  {
    id: "aegis-thurible",
    name: "The Aegis Thurible",
    class: "progression",
    grants: "ward",
    use: { grants: "ward", charges: 3 },
    profile: { grants: { hazardImmune: ["toxic", "rad", "spore", "searing"] }, bias: { hazardWeight: 0.5 } },
  },
  {
    id: "quiescent-metronome",
    name: "The Quiescent Metronome",
    class: "progression",
    grants: "stasis",
    use: { grants: "stasis", charges: 3 },
    profile: { grants: { timeControl: true }, bias: { enableTags: ["moving"] } },
  },
  {
    id: "choral-antiphon",
    name: "The Choral Antiphon",
    class: "progression",
    grants: "resonate",
    use: { grants: "resonate", charges: 3 },
    profile: { bias: { enableTags: ["harmonic", "sonic"] } },
  },
  {
    id: "mockingbell",
    name: "The Mockingbell",
    class: "progression",
    grants: "lure",
    use: { grants: "lure", charges: 3 },
    profile: { bias: { enableTags: ["patrol", "sound-trigger"] } },
  },
  {
    id: "chrysalis-fold",
    name: "The Chrysalis Fold",
    class: "progression",
    grants: "small-form",
    use: { grants: "small-form" },
    profile: { grants: { throughMatter: false }, bias: { enableTags: ["crawlspace", "vent"] }, power: 0.3 },
  },
  {
    id: "mirrorwright-oculus",
    name: "The Mirrorwright Oculus",
    class: "progression",
    grants: "reflect",
    use: { grants: "reflect", charges: 2 },
    profile: { grants: { energyRoute: true }, bias: { enableTags: ["beam", "shield"] } },
  },
  {
    id: "vivisectors-lance",
    name: "The Vivisector's Lance",
    class: "progression",
    grants: "sunder-core",
    use: { grants: "sunder-core", charges: 1 },
    profile: { bias: { enableTags: ["boss"] } },
  },
  {
    id: "gordian-edge",
    name: "The Gordian Edge",
    class: "progression",
    grants: "sever",
    use: { grants: "sever", charges: 1 },
    profile: { bias: { enableTags: ["boss"] } },
  },
];

/** A couple of non-progression items so the useful/filler pass has a pool. */
export const lootCatalog: ItemDef[] = [
  { id: "shard-cache", name: "Astral Shard Cache", class: "filler" },
  { id: "gilded-reliquary", name: "Gilded Reliquary", class: "useful" },
];
