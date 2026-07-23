/**
 * Metroid Prime (2002) reconstruction fixture — encodes a real, shipped ~100-item
 * game's structure to STRESS the schema (not a playable generative dataset). Proves
 * CycleVania's registries hold this scale + shape without a schema change: derived
 * beam combos (`derivedFrom`), a resource-pool capability, a progressive stat line,
 * and the 12-Chozo-Artifact facet-less collectathon key. Sourced from community/
 * randomizer documentation of the GameCube original.
 */

import type { CapabilityDef, GadgetDef, PuzzleDef, ReachTemplatePool, RegistryInput, Facet } from "@cyclevania/core";

const tag = (t: string): Facet => ({ kind: "tag", tag: t });

const majors: CapabilityDef[] = [
  { id: "morph-ball", held: "granted", facets: [tag("morph-tunnel")], powerWeight: () => 0.3 },
  { id: "bombs", held: "granted", facets: [tag("bomb-slot")], powerWeight: () => 0.35 },
  { id: "boost-ball", held: "granted", facets: [tag("half-pipe")], powerWeight: () => 0.5 },
  { id: "spider-ball", held: "granted", facets: [tag("magnetic-track"), { kind: "magnitude", bucket: "traversal.zUp", evaluate: () => 6 }], powerWeight: () => 0.7 },
  { id: "space-jump-boots", held: "granted", facets: [{ kind: "magnitude", bucket: "traversal.zUp", evaluate: () => 4 }], powerWeight: () => 0.7 },
  { id: "grapple-beam", held: "granted", facets: [tag("grapple-point"), { kind: "magnitude", bucket: "traversal.xyGap", evaluate: () => 8 }], powerWeight: () => 0.5 },
  { id: "charge-beam", held: "granted", facets: [], powerWeight: () => 0.3 },
  { id: "wave-beam", held: "granted", facets: [tag("wave-door")], powerWeight: () => 0.4 },
  { id: "ice-beam", held: "granted", facets: [tag("ice-door")], powerWeight: () => 0.5 },
  { id: "plasma-beam", held: "granted", facets: [tag("plasma-door"), { kind: "magnitude", bucket: "challenge.offense", evaluate: () => 0.6 }], powerWeight: () => 0.6 },
  { id: "missile-launcher", held: "granted", facets: [tag("missile-door"), { kind: "magnitude", bucket: "challenge.offense", evaluate: () => 0.4 }], powerWeight: () => 0.4 },
  { id: "varia-suit", held: "granted", facets: [tag("heat-immune"), { kind: "magnitude", bucket: "challenge.defense", evaluate: () => 0.5 }], powerWeight: () => 0.55 },
  { id: "gravity-suit", held: "granted", facets: [tag("water-free")], powerWeight: () => 0.6 },
  { id: "thermal-visor", held: "granted", facets: [tag("thermal")], powerWeight: () => 0.5 },
  { id: "xray-visor", held: "granted", facets: [tag("revealable"), { kind: "magnitude", bucket: "traversal.perceive", evaluate: () => 2 }], powerWeight: () => 0.7 },
  // a resource-pool capability (Power Bombs), and a derived beam combo
  { id: "power-bomb", held: "granted", facets: [tag("destroy-power-bomb-block"), { kind: "resource", poolId: "power-bomb-charge", capacity: (ctx) => 4 + ctx.level, regenHint: "site" }], powerWeight: () => 0.6 },
  { id: "wavebuster", held: { derivedFrom: ["charge-beam", "wave-beam"] }, facets: [{ kind: "magnitude", bucket: "challenge.offense", evaluate: () => 0.7 }], powerWeight: () => 0.5 },
  // a progressive stat line (Energy Tanks — the same id granted many times)
  { id: "energy-tank", held: "granted", facets: [{ kind: "magnitude", bucket: "custom.survivability", evaluate: (ctx) => ctx.level * 100 }], powerWeight: () => 0.2 },
  // the facet-less collectathon key
  { id: "chozo-artifact", held: "granted", facets: [], powerWeight: () => 0.4, guarantee: { withinReachLevels: 6 } },
];

const gadgets: GadgetDef[] = [
  ...majors.filter((c) => c.id !== "chozo-artifact" && c.id !== "energy-tank").map((c) => ({ id: `pickup-${c.id}`, grants: [c.id] })),
  ...Array.from({ length: 14 }, (_, i) => ({ id: `energy-tank-${i}`, grants: ["energy-tank"] })),
  ...Array.from({ length: 12 }, (_, i) => ({ id: `artifact-${i}`, grants: ["chozo-artifact"] })),
];

// puzzles kept optional here so the GENERATIVE stress run never strands; the count
// lock's solvability is unit-proven in the core. A faithful reconstruction pins the
// Artifact Temple as a required world-scope set-piece by template role.
const puzzles: PuzzleDef[] = [
  { id: "flaahgra", scope: "room", class: "optional-reward", condition: { k: "have", cap: "morph-ball" }, outcome: { kind: "grant-capability", capability: "varia-suit" }, powerWeight: () => 0.6, spatialRecipe: "boss-arena", archetype: "boss-puzzle" },
  { id: "artifact-temple", scope: "world", class: "optional-reward", condition: { k: "count", cap: "chozo-artifact", n: 12 }, outcome: { kind: "world-ending" }, powerWeight: () => 0.95, spatialRecipe: "collectathon-shrine" },
];

const pool: ReachTemplatePool = {
  poolAt: () => [
    {
      weight: 1,
      template: {
        id: "mp-region",
        criticalPath: ["hub", "s1", "gate1", "s2", "capstone", "terminal"],
        nodes: {
          hub: { role: "hub", slots: { min: 10, max: 14 } },
          s1: { role: "segment", slots: { min: 3, max: 6 } },
          gate1: { role: "gate", slots: { min: 1, max: 2 } },
          s2: { role: "segment", slots: { min: 3, max: 6 } },
          capstone: { role: "capstone", slots: { min: 1, max: 3 } },
          terminal: { role: "terminal", slots: { min: 0, max: 1 } },
        },
        branches: [{ attachTo: "s1", role: "vault", entrance: "single", slots: { min: 1, max: 3 }, backEdgeChance: 0.4 }],
        gating: { lockFraction: 0.5, compoundChance: 0.2, keepEntryOpen: true, keepExitOpen: true },
        loops: { guaranteeAtLeastOne: true, density: 0.4 },
      },
    },
  ],
};

export const MP_REGISTRY_INPUT: RegistryInput = {
  gadgets: { capabilities: majors, gadgets },
  gadgetEconomy: { min: 2, max: 4 },
  puzzles,
  templatePool: pool,
  lengthPolicy: { min: 7, max: 7 },
  areaCount: { min: 3, max: 3 },
};
