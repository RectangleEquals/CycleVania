/**
 * Assembles the demo registry from the example data packs — the "fire-and-forget"
 * data a game supplies. Also exposes convenience builders that compose a whole
 * world/reach so downstream tools (the inspector, tests) have a one-liner.
 */

import { composeReach, composeWorld, defineRegistry, type Registry } from "@cyclevania/core";
import type { ReachResult, WorldResult } from "@cyclevania/core";
import { gadgetCatalog, lootCatalog } from "./gadget-catalog.js";
import { lockCatalog } from "./lock-catalog.js";
import { demoGeometryKit } from "./demo-geometry-kit.js";
import { demoTemplate } from "./demo-template.js";

export const demoStyles = {
  "sunken-parish": { id: "sunken-parish", biome: "gothic-flooded", tags: ["gothic", "flooded"] },
  "ashen-vault": { id: "ashen-vault", biome: "sci-crypt", tags: ["metal", "ashen"] },
};

export function demoRegistry(): Registry {
  return defineRegistry({
    grid: { areaCellSize: 16, roomCellSize: 2, snap: "ps2" },
    geometryKit: demoGeometryKit,
    items: { catalog: [...gadgetCatalog, ...lootCatalog], startCaps: [] },
    locks: lockCatalog,
    styles: demoStyles,
  });
}

/** Compose one demo Reach. */
export function demoReach(seed: string | number = "cyclevania-demo", reachIndex = 0): ReachResult {
  return composeReach({ registry: demoRegistry(), seed }, { template: demoTemplate, reachIndex });
}

/** Compose a demo World of `reachCount` Reaches, carrying caps forward. */
export function demoWorld(seed: string | number = "cyclevania-demo", reachCount = 3): WorldResult {
  return composeWorld({ registry: demoRegistry(), seed }, { reachCount, template: demoTemplate, carryCaps: true });
}
