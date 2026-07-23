import { describe, it, expect } from "vitest";
import type { CapabilityDef, GadgetDef } from "../capability/index.js";
import { defineRegistry, type RegistryInput } from "../registries/index.js";
import { worldFromRegistry } from "../world/index.js";
import type { ReachRequest } from "../world/index.js";
import type { ReachTemplatePool } from "../template/index.js";
import { assembleReach } from "../descriptors/assemble.js";
import { stableStringify } from "../descriptors/serialize.js";
import { GenError, RegistryError, BudgetError } from "../errors.js";
import { requestReachAsync } from "./async.js";
import { CancellationToken, GenCancelled } from "./cancellation.js";
import { GenerationHorizon } from "./horizon.js";
import { inlineWorker } from "./worker-adapter.js";

const caps: CapabilityDef[] = [
  { id: "jump", held: "granted", facets: [{ kind: "tag", tag: "j" }], powerWeight: () => 0.5 },
  { id: "grapple", held: "granted", facets: [{ kind: "tag", tag: "g" }], powerWeight: () => 0.5 },
];
const gadgets: GadgetDef[] = [{ id: "boots", grants: ["jump"] }, { id: "hook", grants: ["grapple"] }];
const pool: ReachTemplatePool = {
  poolAt: () => [
    {
      weight: 1,
      template: {
        id: "t",
        criticalPath: ["hub", "term"],
        nodes: { hub: { role: "hub", slots: { min: 4, max: 4 } }, term: { role: "terminal", slots: { min: 1, max: 1 } } },
        branches: [],
        gating: { lockFraction: 0, compoundChance: 0, keepEntryOpen: true, keepExitOpen: true },
        loops: { guaranteeAtLeastOne: false, density: 0 },
      },
    },
  ],
};
const makeReg = (over: Partial<RegistryInput> = {}): RegistryInput => ({
  gadgets: { capabilities: caps, gadgets },
  gadgetEconomy: { min: 1, max: 2 },
  complexity: { BaseCeiling: 40, K_MUL: 0.4, K_ADD: 3, TIER_SIZE: 3, JITTER_FRAC: 0.08, LOOKBEHIND_PULL: 0.35, MIN_CEILING: 30, HARD_MAX: 200, ABSOLUTE_HARD_MAX: 300 },
  templatePool: pool,
  ...over,
});
const req = (i: number, from?: number): ReachRequest => (from !== undefined ? { reachIndex: i, fromReachIndex: from, chosenModifiers: [] } : { reachIndex: i, chosenModifiers: [] });

describe("async parity", () => {
  it("async == sync byte-identical, geometry on and off", async () => {
    for (const geometry of [false, true]) {
      const reg = defineRegistry(makeReg());
      const ws = worldFromRegistry(reg, "par", { geometry });
      const wa = worldFromRegistry(reg, "par", { geometry });
      const sync = stableStringify(assembleReach(ws.requestReach(req(0))));
      const async = stableStringify(assembleReach(await requestReachAsync(wa, req(0))));
      expect(async).toBe(sync);
    }
  });
});

describe("cancellation atomicity", () => {
  it("cancelling mid-generation leaves the composer untouched, then a retry succeeds", async () => {
    const w = worldFromRegistry(defineRegistry(makeReg()), "cancel");
    const token = new CancellationToken();
    let progressCount = 0;
    await expect(
      requestReachAsync(w, req(0), {
        token,
        onProgress: () => {
          progressCount++;
          if (progressCount === 2) token.cancel();
        },
      }),
    ).rejects.toBeInstanceOf(GenCancelled);
    expect(w.realized.size).toBe(0); // unchanged
    expect(w.requestLog.length).toBe(0);
    // subsequent uncancelled request succeeds
    const r = await requestReachAsync(w, req(0));
    expect(w.realized.size).toBe(1);
    expect(r.meta.reachIndex).toBe(0);
  });
});

describe("progress", () => {
  it("reports monotonic fractions ending at 1 across every phase", async () => {
    const w = worldFromRegistry(defineRegistry(makeReg()), "prog", { geometry: true });
    const fractions: number[] = [];
    await requestReachAsync(w, req(0), { onProgress: (p) => fractions.push(p.fraction) });
    expect(fractions.length).toBe(8); // one per phase
    for (let i = 1; i < fractions.length; i++) expect(fractions[i]).toBeGreaterThanOrEqual(fractions[i - 1]!);
    expect(fractions[fractions.length - 1]).toBeCloseTo(1, 9);
  });

  it("derives finite elapsed time from an injected clock", async () => {
    const w = worldFromRegistry(defineRegistry(makeReg()), "clock");
    let t = 0;
    const last: number[] = [];
    await requestReachAsync(w, req(0), { now: () => (t += 5), onProgress: (p) => last.push(p.elapsedMs) });
    for (let i = 1; i < last.length; i++) expect(last[i]).toBeGreaterThanOrEqual(last[i - 1]!);
  });
});

describe("horizon", () => {
  it("prefetches ahead, exposes results, and surfaces a choice-required error", async () => {
    const w = worldFromRegistry(defineRegistry(makeReg({ lengthPolicy: { min: 5, max: 5 } })), "horizon");
    w.requestReach(req(0)); // origin
    const h = new GenerationHorizon(w, { ahead: 2, requestFor: (idx) => req(idx, idx - 1), evictBehind: 1 });
    await h.noteAt(0);
    expect(w.realized.has(1)).toBe(true);
    expect(w.realized.has(2)).toBe(true);
    expect(h.get(1)).toBeDefined();

    const wBad = worldFromRegistry(defineRegistry(makeReg({ lengthPolicy: { min: 3, max: 3 } })), "bad");
    wBad.requestReach(req(0));
    const hBad = new GenerationHorizon(wBad, {
      ahead: 1,
      requestFor: () => {
        throw new Error("choice required before this Reach");
      },
    });
    await expect(hBad.noteAt(0)).rejects.toThrow("choice required");
  });
});

describe("error taxonomy + worker parity", () => {
  it("subclasses carry code + structured details and are GenErrors", () => {
    const e = new RegistryError("registry.duplicate-id", "dup", { id: "foo" });
    expect(e).toBeInstanceOf(GenError);
    expect(e).toBeInstanceOf(RegistryError);
    expect(e.code).toBe("registry.duplicate-id");
    expect(e.details).toEqual({ id: "foo" });
    expect(e.name).toBe("RegistryError");
    expect(new BudgetError("finish.poly-budget", "x", { tris: 9 }).details).toEqual({ tris: 9 });
  });

  it("inlineWorker output is byte-identical to the sync core", async () => {
    const reg = defineRegistry(makeReg());
    const w1 = worldFromRegistry(reg, "wk");
    const w2 = worldFromRegistry(reg, "wk");
    const viaWorker = await inlineWorker(w1).run(req(0));
    const direct = w2.requestReach(req(0));
    expect(stableStringify(assembleReach(viaWorker))).toBe(stableStringify(assembleReach(direct)));
  });
});
