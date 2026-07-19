import { describe, it, expect } from "vitest";
import { autosolve, buildSimWorld, composeReach, composeWorld, generateReach, isSolvable } from "@cyclevania/core";
import { demoRegistry } from "./demo-registry.js";
import { demoTemplate } from "./demo-template.js";
import { gadgetCatalog } from "./gadget-catalog.js";

describe("demo soak", () => {
  it("exposes all 24 gadgets as distinct progression capabilities", () => {
    const reg = demoRegistry();
    expect(gadgetCatalog.length).toBe(24);
    expect(reg.items.progression.length).toBe(24);
    expect(new Set(reg.items.progression.map((i) => i.grants)).size).toBe(24);
  });

  it("1000 demo reaches are solvable (zero solver failures)", () => {
    const reg = demoRegistry();
    for (let s = 0; s < 1000; s++) {
      const reach = generateReach({ seed: `soak-${s}`, template: demoTemplate, items: reg.items.progression });
      const byId = new Map(reach.items.map((i) => [i.id, i]));
      expect(isSolvable(reach.graph, reach.placement, byId, reach.startCaps)).toBe(true);
    }
  });

  it("80 fully-composed reaches: the bot completes each end-to-end", () => {
    const reg = demoRegistry();
    for (let s = 0; s < 80; s++) {
      const result = composeReach({ registry: reg, seed: `c-${s}` }, { template: demoTemplate, reachIndex: s % 12 });
      const world = buildSimWorld(result, reg);
      expect(world.terminalAreaId).not.toBeNull();
      expect(autosolve(world).visited.has(world.terminalAreaId as number)).toBe(true);
    }
  }, 30000);

  it("every composed cell resolves to a kit piece or is empty air", () => {
    const result = composeReach({ registry: demoRegistry(), seed: "kit-check" }, { template: demoTemplate, reachIndex: 5 });
    for (const area of result.descriptor.areas) {
      for (const room of area.rooms) {
        for (const cell of room.cells) {
          if (cell.role !== "air") expect(cell.kitId).not.toBeNull();
        }
      }
    }
  });

  it("composeWorld is deterministic (world-parity)", () => {
    const a = composeWorld({ registry: demoRegistry(), seed: "parity" }, { reachCount: 2, template: demoTemplate, carryCaps: true });
    const b = composeWorld({ registry: demoRegistry(), seed: "parity" }, { reachCount: 2, template: demoTemplate, carryCaps: true });
    expect(JSON.stringify(a.descriptor)).toBe(JSON.stringify(b.descriptor));
    expect(a.descriptor.meta.reachCount).toBe(2);
  });

  it("connectors carry real corridor geometry (geometry soak)", () => {
    const reg = demoRegistry();
    for (let s = 0; s < 60; s++) {
      const { descriptor } = composeReach({ registry: reg, seed: `geo-${s}` }, { template: demoTemplate, reachIndex: s % 10 });
      for (const area of descriptor.areas) {
        for (const c of area.connectors) {
          expect(c.cells.length).toBeGreaterThan(0);
          expect(c.origin).toBeDefined();
          expect(c.cellSize).toBeDefined();
          for (const cell of c.cells) expect(["floor", "ceiling", "wall", "opening"]).toContain(cell.role);
        }
      }
    }
  });

  it("capstone areas produce wave-lockdown arena rooms", () => {
    const reg = demoRegistry();
    let found = false;
    for (let s = 0; s < 20 && !found; s++) {
      const { descriptor } = composeReach({ registry: reg, seed: `arena-${s}` }, { template: demoTemplate, reachIndex: 6 });
      for (const area of descriptor.areas) if (area.role === "capstone") for (const room of area.rooms) if (room.arena) { expect(room.arena.waves).toBeGreaterThanOrEqual(1); found = true; }
    }
    expect(found).toBe(true);
  });
});
