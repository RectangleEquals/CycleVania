import { isSolvable, heldFromData, buildSimWorld, autosolve } from "@cyclevania/core";
import type { DiagnosticsConfig } from "@cyclevania/core";
import { parseArgs, strFlag, boolFlag, intFlag } from "../args.js";
import type { CliIO } from "../io.js";
import { resolveSource } from "../sources.js";
import { makeRegistry, realize } from "../pipeline.js";

/** soak <source> --seeds N [--autosolve] [--reaches R] [--seed-prefix P] */
export async function run(rest: string[], io: CliIO, diag: DiagnosticsConfig): Promise<number> {
  const p = parseArgs(rest, ["autosolve"]);
  const source = p._[0];
  if (!source) {
    io.errln("soak: missing <source>");
    return 1;
  }
  const src = await resolveSource(source);
  if (src.kind === "descriptor") {
    io.errln("soak: needs a dataset, not a world descriptor");
    return 1;
  }
  const registry = makeRegistry(src, diag);

  const seeds = Math.max(1, intFlag(p, "seeds", 25));
  const doAutosolve = boolFlag(p, "autosolve");
  const reaches = Math.max(1, intFlag(p, "reaches", 1));
  const prefix = strFlag(p, "seed-prefix") ?? source;

  const start = Date.now();
  let pass = 0;
  let fail = 0;
  let relaxTotal = 0;
  const failures: string[] = [];

  for (let s = 0; s < seeds; s++) {
    const seed = `${prefix}-${s}`;
    let ok = true;
    let reason = "";
    try {
      const { reaches: rr } = realize(registry, src, { seed, reaches });
      for (const r of rr) {
        relaxTotal += r.meta.relaxations.length;
        if (!isSolvable(r.graph, heldFromData(r.meta.startHeld), r.items, r.placement)) {
          ok = false;
          reason = `reach ${r.meta.reachIndex} not solvable`;
          break;
        }
        if (doAutosolve && !autosolve(buildSimWorld(r)).success) {
          ok = false;
          reason = `reach ${r.meta.reachIndex} autosolve failed`;
          break;
        }
      }
    } catch (e) {
      ok = false;
      reason = (e as Error).message;
    }
    if (ok) pass++;
    else {
      fail++;
      failures.push(`  seed ${seed}: ${reason}`);
    }
  }

  const ms = Date.now() - start;
  io.outln(`soak ${source}: ${seeds} seeds × ${reaches} reach(es)${doAutosolve ? " + autosolve" : ""}`);
  io.outln(`  ${pass} pass · ${fail} fail · avg relaxations ${(relaxTotal / Math.max(1, seeds)).toFixed(2)} · ${ms}ms`);
  if (failures.length > 0) {
    io.outln("failures:");
    for (const f of failures.slice(0, 20)) io.outln(f);
  }
  return fail === 0 ? 0 : 1;
}
