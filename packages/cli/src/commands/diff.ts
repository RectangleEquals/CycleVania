import { stableStringify, assembleReach, buildSimWorld, autosolve } from "@cyclevania/core";
import type { DiagnosticsConfig, ReachResult } from "@cyclevania/core";
import { parseArgs, strFlag, intFlag } from "../args.js";
import type { CliIO } from "../io.js";
import { resolveSource, type ResolvedSource } from "../sources.js";
import { makeRegistry, realize } from "../pipeline.js";

function placementMap(reaches: ReachResult[]): Map<string, string> {
  const m = new Map<string, string>();
  for (const r of reaches) for (const [loc, item] of r.placement.entries()) m.set(`r${r.meta.reachIndex}:${loc}`, item);
  return m;
}

function autosolveMoves(reaches: ReachResult[]): number {
  let n = 0;
  for (const r of reaches) {
    const res = autosolve(buildSimWorld(r));
    n += res.state.log.length;
  }
  return n;
}

/** diff <sourceA> <sourceB> [--seed S] [--seedA X] [--seedB Y] [--reaches N] */
export async function run(rest: string[], io: CliIO, diag: DiagnosticsConfig): Promise<number> {
  const p = parseArgs(rest);
  const [aArg, bArg] = p._;
  if (!aArg || !bArg) {
    io.errln("diff: needs <sourceA> <sourceB>");
    return 1;
  }
  const shared = strFlag(p, "seed") ?? "cyclevania";
  const seedA = strFlag(p, "seedA") ?? shared;
  const seedB = strFlag(p, "seedB") ?? shared;
  const reaches = Math.max(1, intFlag(p, "reaches", 1));

  const build = async (arg: string, seed: string): Promise<ReachResult[]> => {
    const src: ResolvedSource = await resolveSource(arg);
    if (src.kind === "descriptor") throw new Error(`diff: "${arg}" is a descriptor — pass regenerable sources`);
    const registry = makeRegistry(src, diag);
    return realize(registry, src, { seed, reaches }).reaches;
  };

  const ra = await build(aArg, seedA);
  const rb = await build(bArg, seedB);

  // whole-world identity first (fast path): compare assembled reach descriptors
  const canon = (rs: ReachResult[]): string => stableStringify(rs.map((r) => assembleReach(r)));
  const wa = canon(ra);
  const wb = canon(rb);
  if (wa === wb) {
    io.outln(`identical — ${aArg}@${seedA} and ${bArg}@${seedB} produce the same world`);
    return 0;
  }

  const pa = placementMap(ra);
  const pb = placementMap(rb);
  const added: string[] = [];
  const removed: string[] = [];
  const moved: string[] = [];
  for (const [loc, item] of pb) {
    if (!pa.has(loc)) added.push(`${loc}=${item}`);
    else if (pa.get(loc) !== item) moved.push(`${loc}: ${pa.get(loc)} → ${item}`);
  }
  for (const loc of pa.keys()) if (!pb.has(loc)) removed.push(loc);

  io.outln(`differ — ${aArg}@${seedA} vs ${bArg}@${seedB}`);
  io.outln(`  placements: +${added.length} added · -${removed.length} removed · ${moved.length} moved`);
  for (const m of moved.slice(0, 20)) io.outln(`    ${m}`);
  io.outln(`  autosolve log length: ${autosolveMoves(ra)} vs ${autosolveMoves(rb)}`);
  return 0;
}
