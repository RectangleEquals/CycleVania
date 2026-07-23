/**
 * The reproduction bundle — the portable "regenerate this exact world" payload,
 * shared by the CLI and the inspector. It CANNOT inline a registry (registries carry
 * callbacks), so it references the dataset by **module specifier + export name** plus
 * inline pure-data generation options, the seed, the request log, and the expected
 * `registryFingerprint`/`generationVersion`. These are pure shapes + a fingerprint
 * check; the dynamic-import loader lives in the CLI/inspector so core imports nothing.
 *
 * Loading (host side): dynamic-import the module, read the export, `defineRegistry`,
 * then `checkReproduction` — a mismatch is a hard, explanatory error (per redesign 02's
 * "what can legitimately change a world": dataset edit, callback revision, engine bump).
 */

import type { WorldFromRegistryOptions } from "../world/index.js";
import type { ReachRequestRecord } from "../world/index.js";
import { GENERATION_VERSION } from "./version.js";

export const BUNDLE_KIND = "cyclevania.reproduction-bundle" as const;

/** How to obtain the callback-bearing dataset. `export` is a `Preset` (has `.input`) or a raw `RegistryInput`. */
export interface BundleRegistryRef {
  module: string;
  export: string;
}

export interface ReproductionBundle {
  kind: typeof BUNDLE_KIND;
  formatVersion: 1;
  registry: BundleRegistryRef;
  seed: string;
  /** Pure-data generation options (fidelity / dials / geometry flag / …) — all JSON-safe. */
  world?: WorldFromRegistryOptions;
  /** The realized request sequence to replay (empty ⇒ caller decides count). */
  requestLog: ReachRequestRecord[];
  expected: { registryFingerprint: string; generationVersion: string };
}

export interface MakeBundleArgs {
  registry: BundleRegistryRef;
  seed: string;
  world?: WorldFromRegistryOptions;
  requestLog: ReachRequestRecord[];
  registryFingerprint: string;
}

/** Build a bundle from a completed generation (keys emitted in a stable order). */
export function makeBundle(args: MakeBundleArgs): ReproductionBundle {
  const b: ReproductionBundle = {
    kind: BUNDLE_KIND,
    formatVersion: 1,
    registry: { module: args.registry.module, export: args.registry.export },
    seed: args.seed,
    requestLog: args.requestLog,
    expected: { registryFingerprint: args.registryFingerprint, generationVersion: GENERATION_VERSION },
  };
  if (args.world !== undefined) b.world = args.world;
  return b;
}

export interface ReproCheck {
  ok: boolean;
  expectedFingerprint: string;
  actualFingerprint: string;
  expectedVersion: string;
  actualVersion: string;
  reason?: "fingerprint" | "version";
  message: string;
}

/**
 * Compare a bundle's expectations against a freshly-built registry + this engine.
 * Explains WHAT changed rather than just failing, so a host can act on it.
 */
export function checkReproduction(bundle: ReproductionBundle, actual: { registryFingerprint: string; generationVersion: string }): ReproCheck {
  const fpOk = bundle.expected.registryFingerprint === actual.registryFingerprint;
  const verOk = bundle.expected.generationVersion === actual.generationVersion;
  const base = {
    expectedFingerprint: bundle.expected.registryFingerprint,
    actualFingerprint: actual.registryFingerprint,
    expectedVersion: bundle.expected.generationVersion,
    actualVersion: actual.generationVersion,
  };
  if (fpOk && verOk) return { ...base, ok: true, message: "reproduction verified — fingerprint and engine version match" };
  if (!fpOk) {
    return {
      ...base,
      ok: false,
      reason: "fingerprint",
      message:
        `dataset fingerprint changed: expected ${bundle.expected.registryFingerprint}, got ${actual.registryFingerprint}. ` +
        `The referenced module '${bundle.registry.module}#${bundle.registry.export}' was edited (data or a callback revision bump) since export — the world will differ.`,
    };
  }
  return {
    ...base,
    ok: false,
    reason: "version",
    message: `generation engine changed: bundle built with ${bundle.expected.generationVersion}, this engine is ${actual.generationVersion}. Output may differ across a version bump.`,
  };
}

export type PayloadKind = "reproduction-bundle" | "world-descriptor" | "dataset" | "unknown";

/** Classify a parsed JSON payload by shape (bundle / descriptor / dataset). */
export function sniffPayload(x: unknown): PayloadKind {
  if (x === null || typeof x !== "object") return "unknown";
  const o = x as Record<string, unknown>;
  if (o["kind"] === BUNDLE_KIND) return "reproduction-bundle";
  const meta = o["meta"] as Record<string, unknown> | undefined;
  if (meta && typeof meta["generationVersion"] === "string" && Array.isArray(o["reaches"])) return "world-descriptor";
  const gadgets = o["gadgets"] as Record<string, unknown> | undefined;
  if (gadgets && Array.isArray(gadgets["capabilities"])) return "dataset";
  return "unknown";
}
