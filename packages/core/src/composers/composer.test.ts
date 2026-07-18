import { describe, it, expect } from "vitest";
import { defineRegistry } from "../registries/registry.js";
import type { GeometryKit } from "../registries/geometry-kit.js";
import type { ReachTemplate } from "../template/template.js";
import { composeReach } from "./reach-composer.js";
import { composeWorld } from "./world-composer.js";

const rad = (d: number): number => (d * Math.PI) / 180;

const KIT: GeometryKit = {
  pieces: [
    { id: "floor", role: "floor", snapAngles: [0], tags: ["floor"], collider: "solid" },
    { id: "ceil", role: "ceiling", snapAngles: [0], tags: ["ceiling"], collider: "solid" },
    { id: "wall", role: "wall", snapAngles: [0, rad(90), rad(180), rad(270)], tags: ["wall"], collider: "solid" },
    { id: "corner", role: "corner", snapAngles: [0, rad(45)], tags: ["corner"], collider: "solid" },
    { id: "door", role: "opening", snapAngles: [0], tags: ["opening"], socketCapable: true, collider: "none" },
  ],
};

const TEMPLATE: ReachTemplate = {
  criticalPath: ["hub", "seg1", "seg2", "seg3", "seg4", "seg5", "capstone", "terminal"],
  nodes: {
    hub: { id: "hub", role: "hub", slots: { min: 3, max: 4 }, bootstrap: true },
    seg1: { id: "seg1", role: "segment", slots: { min: 1, max: 2 } },
    seg2: { id: "seg2", role: "segment", slots: { min: 1, max: 2 } },
    seg3: { id: "seg3", role: "segment", slots: { min: 1, max: 2 } },
    seg4: { id: "seg4", role: "segment", slots: { min: 1, max: 2 } },
    seg5: { id: "seg5", role: "segment", slots: { min: 1, max: 2 } },
    capstone: { id: "capstone", role: "capstone", slots: { min: 1, max: 1 } },
    terminal: { id: "terminal", role: "terminal", slots: { min: 1, max: 1 } },
  },
  branches: [{ anchor: "any-segment", role: "vault", slots: { min: 1, max: 2 }, entrance: "compound", backEdge: { chance: 0.6, toEarlier: true, gated: 0.5 } }],
  gating: { lockFraction: 0.5, compoundChance: 0.25, keepEntryOpen: true, keepExitOpen: true },
  loops: { guaranteeAtLeastOne: true },
};

function reg() {
  return defineRegistry({
    grid: { areaCellSize: 16, roomCellSize: 2, snap: "ps2" },
    geometryKit: KIT,
    items: {
      catalog: [
        { id: "skyhook", class: "progression", grants: "grapple" },
        { id: "lantern", class: "progression", grants: "reveal" },
        { id: "censer", class: "progression", grants: "leap" },
      ],
      startCaps: [],
    },
    locks: { gate: (r) => r.have("grapple") },
    styles: { sunken: { id: "sunken" } },
  });
}

describe("composeReach", () => {
  it("produces one area per region with a valid subdivided cell grid", () => {
    const registry = reg();
    const { reach, descriptor } = composeReach({ registry, seed: "world-7" }, { template: TEMPLATE, reachIndex: 2 });
    expect(descriptor.areas.length).toBe(reach.graph.regions.size);

    for (const area of descriptor.areas) {
      expect(area.rooms.length).toBeGreaterThan(0);
      for (const room of area.rooms) {
        expect(room.cells.length).toBe(room.footprint[0] * room.footprint[1] * room.footprint[2]);
        // every non-air cell resolves to a kit piece (kit is complete)
        for (const cell of room.cells) {
          if (cell.role !== "air") expect(cell.kitId).not.toBeNull();
        }
      }
    }
  });

  it("carries gate rules onto portals (full requiredCaps, never collapsed)", () => {
    const { descriptor } = composeReach({ registry: reg(), seed: "gates" }, { template: TEMPLATE, reachIndex: 3 });
    const gated = descriptor.areas.flatMap((a) => a.portals).filter((p) => p.requires);
    // there ARE gated edges in this template, and each carries its full cap set
    expect(gated.length).toBeGreaterThan(0);
    for (const p of gated) expect((p.requiredCaps ?? []).length).toBeGreaterThan(0);
  });

  it("places exactly the progression items as gadgets", () => {
    const { descriptor } = composeReach({ registry: reg(), seed: "gadgets" }, { template: TEMPLATE });
    const gadgets = descriptor.areas.flatMap((a) => a.gadgets);
    expect(gadgets.length).toBe(3); // three progression items
    expect(new Set(gadgets.map((g) => g.itemId)).size).toBe(3);
  });

  it("is deterministic", () => {
    const a = composeReach({ registry: reg(), seed: "det" }, { template: TEMPLATE, reachIndex: 1 });
    const b = composeReach({ registry: reg(), seed: "det" }, { template: TEMPLATE, reachIndex: 1 });
    expect(JSON.stringify(a.descriptor)).toBe(JSON.stringify(b.descriptor));
  });
});

describe("composeWorld", () => {
  it("sequences reaches and carries caps forward", () => {
    const world = composeWorld({ registry: reg(), seed: "run" }, { reachCount: 3, template: TEMPLATE, carryCaps: true });
    expect(world.reaches.length).toBe(3);
    for (const r of world.reaches) expect(r.descriptor.areas.length).toBeGreaterThan(0);
  });
});
