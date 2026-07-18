import { describe, it, expect } from "vitest";
import { defineRegistry, type RegistryInput } from "./registry.js";
import type { GeometryKit } from "./geometry-kit.js";

const KIT: GeometryKit = { pieces: [{ id: "floor", role: "Floor", snapAngles: [0], tags: ["floor"], collider: "solid" }] };

function base(): RegistryInput {
  return {
    grid: { areaCellSize: 16, roomCellSize: 2, snap: "ps2" },
    geometryKit: KIT,
    items: { catalog: [{ id: "skyhook", class: "progression", grants: "grapple" }], startCaps: [] },
    locks: { chasm: (r) => r.have("grapple"), vault: { solvedBy: (r) => r.and(r.have("grapple"), r.have("reveal")), recipe: "hidden-crossing" } },
  };
}

describe("defineRegistry", () => {
  it("resolves lock builders to concrete rules with a default recipe", () => {
    const reg = defineRegistry(base());
    const lock = reg.locks.get("chasm");
    expect(lock?.rule).toEqual({ k: "have", cap: "grapple" });
    expect(lock?.recipe).toBe("gate");
  });

  it("keeps an explicit ChallengeTemplate's recipe", () => {
    const reg = defineRegistry(base());
    expect(reg.locks.get("vault")?.recipe).toBe("hidden-crossing");
  });

  it("throws on a progression item without grants", () => {
    const input = base();
    input.items = { catalog: [{ id: "x", class: "progression" }], startCaps: [] };
    expect(() => defineRegistry(input)).toThrow(/must declare/);
  });

  it("throws on duplicate item ids", () => {
    const input = base();
    input.items = { catalog: [{ id: "x", class: "filler" }, { id: "x", class: "filler" }] };
    expect(() => defineRegistry(input)).toThrow(/duplicate/);
  });

  it("exposes the progression subset for the solver", () => {
    const reg = defineRegistry(base());
    expect(reg.items.progression).toEqual([{ id: "skyhook", grants: "grapple" }]);
  });
});
