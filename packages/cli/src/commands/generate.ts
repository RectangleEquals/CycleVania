import { writeFileSync } from "node:fs";
import { assembleWorld, stableStringify, makeBundle, checkReproduction, GENERATION_VERSION } from "@cyclevania/core";
import type { DiagnosticsConfig } from "@cyclevania/core";
import { parseArgs, strFlag, boolFlag, intFlag } from "../args.js";
import type { CliIO } from "../io.js";
import { resolveSource } from "../sources.js";
import { makeRegistry, realize, type RealizeOptions } from "../pipeline.js";

/** generate <source> [--seed S] [--reaches N] [--geometry] [-o out.json] [--save-bundle b.json] */
export async function run(rest: string[], io: CliIO, diag: DiagnosticsConfig): Promise<number> {
  const p = parseArgs(rest, ["geometry"]);
  const source = p._[0];
  if (!source) {
    io.errln("generate: missing <source>");
    return 1;
  }
  const src = await resolveSource(source);
  if (src.kind === "descriptor") {
    io.errln("generate: that file is a world descriptor (already generated) — pass a preset, module#export, or bundle");
    return 1;
  }
  const registry = makeRegistry(src, diag);

  if (src.bundle) {
    const chk = checkReproduction(src.bundle, { registryFingerprint: registry.fingerprint, generationVersion: GENERATION_VERSION });
    if (!chk.ok) {
      io.errln(`generate: reproduction check failed — ${chk.message}`);
      return 1;
    }
  }

  const seed = strFlag(p, "seed") ?? src.seed ?? "cyclevania";
  const ropts: RealizeOptions = { seed };
  if (boolFlag(p, "geometry")) ropts.geometry = true;
  if (strFlag(p, "reaches") !== undefined) ropts.reaches = Math.max(1, intFlag(p, "reaches", 1));

  const { world, usedWorldOptions } = realize(registry, src, ropts);

  const text = stableStringify(assembleWorld(world));
  const out = strFlag(p, "out");
  if (out) {
    writeFileSync(out, text, "utf8");
    io.errln(`generate: wrote ${world.realized.size} Reach(es) → ${out}`);
  } else {
    io.outln(text);
  }

  const bundleOut = strFlag(p, "save-bundle");
  if (bundleOut) {
    if (!src.ref) {
      io.errln("generate: cannot save a bundle for this source (no module reference)");
      return 1;
    }
    const bundle = makeBundle({ registry: src.ref, seed, world: usedWorldOptions, requestLog: world.requestLog, registryFingerprint: registry.fingerprint });
    writeFileSync(bundleOut, stableStringify(bundle), "utf8");
    io.errln(`generate: wrote reproduction bundle → ${bundleOut}`);
  }
  return 0;
}
