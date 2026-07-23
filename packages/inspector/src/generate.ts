/**
 * Generation driver — builds a validated registry from the chosen preset, realizes
 * N chained Reaches through `requestReachAsync` (so the progress overlay + diagnostics
 * panel get a real stream), and stores the results. A coarse fidelity keeps the
 * geometry demo snappy in the browser without changing the preset's character much.
 */

import { defineRegistry, worldFromRegistry, requestReachAsync, assembleWorld, MemorySink } from "@cyclevania/core";
import type { ReachResult, FidelityProfile, WorldFromRegistryOptions } from "@cyclevania/core";
import { PRESETS } from "@cyclevania/examples";
import { store } from "./state.js";

const DEMO_FIDELITY: FidelityProfile = { angleStepDeg: 5, voxelRes: 2.4, maxDim: 28, snapNormals: true };

export async function regenerate(): Promise<void> {
  const { presetName, seed, reaches, geometry } = store.state.settings;
  const preset = PRESETS[presetName] ?? PRESETS["classic"];
  if (!preset) throw new Error(`unknown preset "${presetName}"`);

  const sink = new MemorySink();
  const registry = defineRegistry({ ...preset.input, diagnostics: { level: store.state.diagLevel === "warn" ? "debug" : store.state.diagLevel, sink } });

  const worldOpts: WorldFromRegistryOptions = { ...preset.world, geometry, ...(geometry ? { fidelity: DEMO_FIDELITY } : {}) };
  const world = worldFromRegistry(registry, seed, worldOpts);

  store.set({ world, reaches: [], descriptor: undefined, generating: true, diagnostics: [], selection: { kind: "none" }, sphereStep: -1 });

  const realized: ReachResult[] = [];
  try {
    for (let i = 0; i < reaches; i++) {
      if (world.drawnLength !== undefined && i >= world.drawnLength) break;
      const req = i === 0 ? { reachIndex: 0, chosenModifiers: [] } : { reachIndex: i, fromReachIndex: i - 1, chosenModifiers: [] };
      const rr = await requestReachAsync(world, req, { onProgress: (p) => store.set({ progress: p }) });
      realized.push(rr);
    }
    store.set({ reaches: realized, descriptor: assembleWorld(world), diagnostics: [...sink.events], generating: false, progress: undefined });
  } catch (e) {
    sink.emit({ level: "error", code: "inspector.generate-failed", message: (e as Error).message });
    store.set({ reaches: realized, diagnostics: [...sink.events], generating: false, progress: undefined });
    throw e;
  }
}
