import { describe, it, expect } from "vitest";
import { demoReach } from "./demo-registry.js";

// this package targets no host lib; the test runner provides console at runtime
declare const console: { log(...args: unknown[]): void };

describe("room richness", () => {
  it("rooms have a walkable interior void (never floor-directly-under-ceiling)", () => {
    const { descriptor } = demoReach("shape", 6);
    for (const a of descriptor.areas) {
      for (const r of a.rooms) {
        const midZ = Math.floor(r.footprint[2] / 2);
        const interiorAir = r.cells.filter((c) => c.coord[2] === midZ && c.role === "air").length;
        expect(r.footprint[2]).toBeGreaterThanOrEqual(3);
        expect(interiorAir).toBeGreaterThan(0);
      }
    }
  });

  it("some rooms have non-rectangular (carved) footprints", () => {
    const { descriptor } = demoReach("shape", 6);
    let carved = 0;
    for (const a of descriptor.areas) {
      for (const r of a.rooms) {
        const floorAir = r.cells.filter((c) => c.coord[2] === 0 && c.role === "air").length;
        if (floorAir > 0) carved++;
      }
    }
    expect(carved).toBeGreaterThan(0); // round/cavern rooms carve the box outline
  });

  it("prints a floor plan for eyeballing", () => {
    const { descriptor } = demoReach("shape", 6);
    let best = descriptor.areas[0]!.rooms[0]!;
    for (const a of descriptor.areas) for (const r of a.rooms) if (r.cells.length > best.cells.length) best = r;
    const [ex, ey] = best.footprint;
    const roleAt = new Map<string, string>();
    for (const c of best.cells) roleAt.set(c.coord.join(","), c.role);
    const glyph: Record<string, string> = { floor: "#", wall: "@", corner: "@", opening: "O", ceiling: ".", air: " " };
    const lines: string[] = [`room ${best.nodeId} kind=${best.kind} footprint=${best.footprint.join("x")} cells=${best.cells.length}`];
    for (let y = ey - 1; y >= 0; y--) {
      let line = "  ";
      for (let x = 0; x < ex; x++) line += glyph[roleAt.get(`${x},${y},0`) ?? "air"] ?? "?";
      lines.push(line);
    }
    console.log("\n" + lines.join("\n"));
    expect(best.cells.length).toBeGreaterThan(0);
  });
});
