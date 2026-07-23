/**
 * Source resolution — turn a `<source>` CLI argument into something generatable.
 * Accepts: a built-in preset alias (`classic`/`crawler`/`prime`/`mp`), a
 * `module#export` reference, a reproduction-bundle JSON file, or a world-descriptor
 * JSON file (read-only). Callback-bearing datasets are obtained by **dynamic import**
 * (module specifier + export name) so `core` itself never imports a dataset.
 */

import { readFileSync, existsSync } from "node:fs";
import { sniffPayload } from "@cyclevania/core";
import type { RegistryInput, WorldFromRegistryOptions, BundleRegistryRef, ReproductionBundle, WorldDescriptor, ReachRequestRecord } from "@cyclevania/core";

export const PRESET_ALIASES: Record<string, BundleRegistryRef> = {
  classic: { module: "@cyclevania/examples", export: "CLASSIC" },
  crawler: { module: "@cyclevania/examples", export: "CRAWLER" },
  prime: { module: "@cyclevania/examples", export: "PRIME" },
  mp: { module: "@cyclevania/examples", export: "MP_REGISTRY_INPUT" },
};

export interface ResolvedSource {
  kind: "registry" | "descriptor";
  input?: RegistryInput;
  world?: WorldFromRegistryOptions;
  ref?: BundleRegistryRef;
  seed?: string;
  requestLog?: ReachRequestRecord[];
  bundle?: ReproductionBundle;
  descriptor?: WorldDescriptor;
}

/** A Preset bundles a RegistryInput under `.input` + world dials under `.world`. */
function isPreset(x: unknown): x is { input: RegistryInput; world?: WorldFromRegistryOptions } {
  return typeof x === "object" && x !== null && "input" in x;
}

/** Dynamic-import a dataset by module + export, normalizing Preset vs raw RegistryInput. */
export async function loadRef(ref: BundleRegistryRef): Promise<{ input: RegistryInput; world: WorldFromRegistryOptions }> {
  let mod: Record<string, unknown>;
  try {
    mod = (await import(ref.module)) as Record<string, unknown>;
  } catch (e) {
    throw new Error(`cannot import dataset module "${ref.module}": ${(e as Error).message}`);
  }
  const ex = mod[ref.export];
  if (ex === undefined) throw new Error(`module "${ref.module}" has no export "${ref.export}"`);
  if (isPreset(ex)) return { input: ex.input, world: ex.world ?? {} };
  return { input: ex as RegistryInput, world: {} };
}

export async function resolveSource(arg: string): Promise<ResolvedSource> {
  const alias = PRESET_ALIASES[arg.toLowerCase()];
  if (alias) {
    const { input, world } = await loadRef(alias);
    return { kind: "registry", input, world, ref: alias };
  }

  if (existsSync(arg)) {
    let json: unknown;
    try {
      json = JSON.parse(readFileSync(arg, "utf8"));
    } catch (e) {
      throw new Error(`"${arg}" is not valid JSON: ${(e as Error).message}`);
    }
    const kind = sniffPayload(json);
    if (kind === "reproduction-bundle") {
      const bundle = json as ReproductionBundle;
      const { input, world } = await loadRef(bundle.registry);
      const merged: WorldFromRegistryOptions = { ...world, ...(bundle.world ?? {}) };
      return { kind: "registry", input, world: merged, ref: bundle.registry, seed: bundle.seed, requestLog: bundle.requestLog, bundle };
    }
    if (kind === "world-descriptor") return { kind: "descriptor", descriptor: json as WorldDescriptor };
    if (kind === "dataset") throw new Error(`"${arg}" is a standalone dataset file — merge it in the inspector; the CLI takes a preset, a module#export, or a bundle`);
    throw new Error(`"${arg}" has an unrecognized shape (not a bundle, descriptor, or dataset)`);
  }

  if (arg.includes("#")) {
    const hash = arg.indexOf("#");
    const ref: BundleRegistryRef = { module: arg.slice(0, hash), export: arg.slice(hash + 1) };
    const { input, world } = await loadRef(ref);
    return { kind: "registry", input, world, ref };
  }

  throw new Error(`unknown source "${arg}" — expected a preset (${Object.keys(PRESET_ALIASES).join("/")}), a module#export, or a JSON file path`);
}
