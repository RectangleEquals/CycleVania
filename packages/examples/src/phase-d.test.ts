import { describe, it, expect } from "vitest";
import { composeReach, composeWorld, isQuantized, itemPower, type Vec3 } from "@cyclevania/core";
import { demoRegistry } from "./demo-registry.js";
import { demoTemplate } from "./demo-template.js";

describe("Phase D — generated geometry", () => {
  const reg = demoRegistry();
  const built = composeReach({ registry: reg, seed: "geo-demo" }, { template: demoTemplate, reachIndex: 0, geometry: true });

  it("attaches a dedup'd kit + instances + occupancy to every area", () => {
    let sawGeometry = false;
    for (const area of built.descriptor.areas) {
      if (!area.kit) continue;
      sawGeometry = true;
      expect(area.instances && area.instances.length).toBeGreaterThan(0);
      // dedup: unique pieces ≪ placements
      expect(area.kit.pieces.length).toBeLessThan((area.instances as unknown[]).length);
      expect(area.occupancy).toBeDefined();
      expect(area.occupancy!.solid.length).toBe(area.occupancy!.dims[0] * area.occupancy!.dims[1] * area.occupancy!.dims[2]);
      // every instance references a real piece
      const ids = new Set(area.kit.pieces.map((p) => p.id));
      expect((area.instances as { pieceId: string }[]).every((i) => ids.has(i.pieceId))).toBe(true);
    }
    expect(sawGeometry).toBe(true);
  });

  it("every generated normal lies on the 5° fidelity grid", () => {
    for (const area of built.descriptor.areas) {
      for (const piece of area.kit?.pieces ?? []) {
        for (let i = 0; i < piece.normals.length; i += 3) {
          expect(isQuantized([piece.normals[i]!, piece.normals[i + 1]!, piece.normals[i + 2]!] as Vec3, 1e-2)).toBe(true);
        }
      }
    }
  });

  it("is deterministic (identical geometry buffers across two runs)", () => {
    const a = composeReach({ registry: demoRegistry(), seed: "geo-det" }, { template: demoTemplate, reachIndex: 1, geometry: true });
    const b = composeReach({ registry: demoRegistry(), seed: "geo-det" }, { template: demoTemplate, reachIndex: 1, geometry: true });
    expect(JSON.stringify(a.descriptor.areas.map((x) => x.kit))).toBe(JSON.stringify(b.descriptor.areas.map((x) => x.kit)));
  });

  it("occasionally produces large / outdoor rooms across seeds", () => {
    let large = 0;
    let outdoor = 0;
    for (let s = 0; s < 24; s++) {
      const r = composeReach({ registry: reg, seed: `budget-${s}` }, { template: demoTemplate, reachIndex: 5, geometry: true });
      for (const area of r.descriptor.areas)
        for (const room of area.rooms) {
          if (room.footprint[0] >= 14 || room.footprint[1] >= 14) large++;
          if (room.outdoor) outdoor++;
        }
    }
    expect(large).toBeGreaterThan(0);
    expect(outdoor).toBeGreaterThan(0);
  });
});

describe("Phase D — organic gadget economy", () => {
  const reg = demoRegistry();
  const max = reg.gadgetEconomy.max;

  function gadgetAnchorsByArea(seed: string) {
    const built = composeReach({ registry: reg, seed }, { template: demoTemplate, reachIndex: 2 });
    return built.descriptor.areas.map((area) => {
      const perRoom = area.rooms.map((room) => room.cells.reduce((n, c) => n + c.contents.filter((x) => x.kind === "gadget").length, 0));
      return { roomCount: area.rooms.length, perRoom, total: perRoom.reduce((a, b) => a + b, 0) };
    });
  }

  it("places ≥1 gadget per reach, ≤1 per room, never in a multi-room entry room", () => {
    for (let s = 0; s < 30; s++) {
      const areas = gadgetAnchorsByArea(`econ-${s}`);
      const reachTotal = areas.reduce((n, a) => n + a.total, 0);
      expect(reachTotal).toBeGreaterThanOrEqual(1);
      for (const a of areas) {
        for (const g of a.perRoom) expect(g).toBeLessThanOrEqual(1); // ≤1 gadget per room
        if (a.roomCount > 1 && a.total > 0) expect(a.perRoom[0]).toBe(0); // not the entry room
      }
    }
  });

  it("selects at most `max` gadgets per reach", () => {
    for (let s = 0; s < 20; s++) {
      const built = composeReach({ registry: reg, seed: `count-${s}` }, { template: demoTemplate, reachIndex: 3 });
      const placed = built.reach.items.length;
      expect(placed).toBeGreaterThanOrEqual(1);
      expect(placed).toBeLessThanOrEqual(max);
    }
  });

  it("weights early reaches toward lateral (low-power) gadgets vs late reaches", () => {
    const reachCount = 6;
    let earlyPower = 0;
    let earlyN = 0;
    let latePower = 0;
    let lateN = 0;
    for (let s = 0; s < 40; s++) {
      const world = composeWorld({ registry: reg, seed: `sched-${s}` }, { reachCount, template: demoTemplate });
      const powerOf = (r: (typeof world.reaches)[number]): number[] => r.reach.items.map((it) => itemPower(reg.items.defs.get(it.id)));
      for (const p of powerOf(world.reaches[0]!)) {
        earlyPower += p;
        earlyN++;
      }
      for (const p of powerOf(world.reaches[reachCount - 1]!)) {
        latePower += p;
        lateN++;
      }
    }
    const earlyAvg = earlyPower / Math.max(1, earlyN);
    const lateAvg = latePower / Math.max(1, lateN);
    expect(lateAvg).toBeGreaterThan(earlyAvg); // schedule shifts power up with depth
  });
});
