import { describe, it, expect } from "vitest";
import { defineRegistry, worldFromRegistry, buildSimWorld, autosolve } from "@cyclevania/core";
import { CRAWLER, CLASSIC, PRIME, PRESETS, presetWorld, type Preset } from "./index.js";
import { MP_REGISTRY_INPUT } from "./fixtures/mp.js";

const PRESET_LIST: Preset[] = [CRAWLER, CLASSIC, PRIME];

describe("presets — registry validity", () => {
  it("every preset + the MP fixture pass defineRegistry", () => {
    for (const p of PRESET_LIST) expect(() => defineRegistry(p.input)).not.toThrow();
    expect(() => defineRegistry(MP_REGISTRY_INPUT)).not.toThrow();
  });

  it("fingerprints are stable + distinct per preset", () => {
    const fps = PRESET_LIST.map((p) => defineRegistry(p.input).fingerprint);
    // stable: same input → same fingerprint
    for (const p of PRESET_LIST) expect(defineRegistry(p.input).fingerprint).toBe(defineRegistry(p.input).fingerprint);
    // distinct: the three presets differ in content
    expect(new Set(fps).size).toBe(3);
  });

  it("PRESETS map is keyed by name", () => {
    for (const [name, p] of Object.entries(PRESETS)) expect(p.name).toBe(name);
  });
});

describe("presets — solvability soak (geometry off)", () => {
  for (const preset of PRESET_LIST) {
    it(`${preset.name}: 75 seeds each generate + autosolve to terminal`, () => {
      for (let seed = 0; seed < 75; seed++) {
        const w = presetWorld(preset, `${preset.name}-${seed}`);
        const rr = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
        const res = autosolve(buildSimWorld(rr)); // throws if stuck despite isSolvable
        expect(res.success).toBe(true);
      }
    });
  }

  it("chains three carried Reaches per preset without stranding", () => {
    for (const preset of PRESET_LIST) {
      const w = presetWorld(preset, `${preset.name}-chain`);
      const r0 = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
      const r1 = w.requestReach({ reachIndex: 1, fromReachIndex: 0, chosenModifiers: [] });
      const r2 = w.requestReach({ reachIndex: 2, fromReachIndex: 1, chosenModifiers: [] });
      for (const rr of [r0, r1, r2]) expect(autosolve(buildSimWorld(rr)).success).toBe(true);
      // caps accumulate across the chain
      expect(Object.keys(r2.meta.startHeld.caps).length).toBeGreaterThanOrEqual(Object.keys(r1.meta.startHeld.caps).length);
    }
  });
});

describe("presets — spatial character", () => {
  it("crawler stays boxy: no outdoor Spaces, no landmarks, low z-spread", () => {
    for (let seed = 0; seed < 30; seed++) {
      const w = presetWorld(CRAWLER, `crawler-char-${seed}`);
      const rr = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
      expect(rr.skeleton.landmarkSpaceIds).toHaveLength(0);
      expect(rr.skeleton.areas.some((a) => a.spaces.some((s) => s.outdoor))).toBe(false);
    }
  });

  it("prime flexes: landmarks always, outdoor Spaces appear across seeds", () => {
    let sawOutdoor = false;
    for (let seed = 0; seed < 30; seed++) {
      const w = presetWorld(PRIME, `prime-char-${seed}`);
      const rr = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
      expect(rr.skeleton.landmarkSpaceIds.length).toBeGreaterThanOrEqual(1);
      if (rr.skeleton.areas.some((a) => a.spaces.some((s) => s.outdoor))) sawOutdoor = true;
    }
    expect(sawOutdoor).toBe(true);
  });

  it("classic places exactly one landmark per Reach", () => {
    for (let seed = 0; seed < 20; seed++) {
      const w = presetWorld(CLASSIC, `classic-char-${seed}`);
      const rr = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
      expect(rr.skeleton.landmarkSpaceIds).toHaveLength(1);
    }
  });
});

describe("presets — geometry on (smoke)", () => {
  it("classic with geometry produces a deduplicated, finite kit", () => {
    const w = presetWorld(CLASSIC, "geo-smoke", { geometry: true });
    const rr = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
    const finished = rr.skeleton.areas.filter((a) => a.finish);
    expect(finished.length).toBeGreaterThan(0);
    for (const a of finished) {
      const f = a.finish!;
      expect(f.instances.length).toBeGreaterThan(0);
      // dedup: unique pieces never exceed the number of placements
      expect(f.kit.pieces.length).toBeLessThanOrEqual(f.instances.length);
      // no NaN/Inf leaked into any buffer
      for (const p of f.kit.pieces) for (const v of p.positions) expect(Number.isFinite(v)).toBe(true);
    }
    // autosolve still holds with geometry on
    expect(autosolve(buildSimWorld(rr)).success).toBe(true);
  });
});

describe("MP fixture — schema stress", () => {
  it("holds derived combos, a resource pool, a progressive line, and the 12-artifact key", () => {
    const reg = defineRegistry(MP_REGISTRY_INPUT);
    expect(reg.capById.get("wavebuster")?.held).not.toBe("granted"); // derivedFrom
    expect(reg.gadgets.filter((g) => g.grants.includes("chozo-artifact"))).toHaveLength(12);
    expect(reg.gadgets.filter((g) => g.grants.includes("energy-tank"))).toHaveLength(14);
    const powerBomb = reg.capById.get("power-bomb");
    expect(powerBomb?.facets.some((f) => f.kind === "resource")).toBe(true);
  });

  it("generates solvable Reaches across 40 seeds", () => {
    const reg = defineRegistry(MP_REGISTRY_INPUT);
    for (let seed = 0; seed < 40; seed++) {
      const w = worldFromRegistry(reg, `mp-${seed}`);
      const rr = w.requestReach({ reachIndex: 0, chosenModifiers: [] });
      expect(autosolve(buildSimWorld(rr)).success).toBe(true);
    }
  });
});
