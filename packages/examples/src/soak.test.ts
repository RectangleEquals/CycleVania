import { describe, it, expect } from "vitest";
import { autosolve, buildSimWorld, composeReach, generateReach, isSolvable } from "@cyclevania/core";
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

  it("300 fully-composed reaches: the bot completes each end-to-end", () => {
    const reg = demoRegistry();
    for (let s = 0; s < 300; s++) {
      const result = composeReach({ registry: reg, seed: `c-${s}` }, { template: demoTemplate, reachIndex: s % 12 });
      const world = buildSimWorld(result, reg);
      expect(world.terminalAreaId).not.toBeNull();
      expect(autosolve(world).visited.has(world.terminalAreaId as number)).toBe(true);
    }
  });

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
});
