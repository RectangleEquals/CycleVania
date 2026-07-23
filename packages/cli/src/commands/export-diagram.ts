import { writeFileSync } from "node:fs";
import { assembleReach, reachMissionDiagram } from "@cyclevania/core";
import type { DiagnosticsConfig } from "@cyclevania/core";
import { parseArgs, strFlag, intFlag } from "../args.js";
import type { CliIO } from "../io.js";
import { resolveSource } from "../sources.js";
import { makeRegistry, realize } from "../pipeline.js";

/** export-diagram <source> --seed S --view mission [--reach I] [-o out.mmd] */
export async function run(rest: string[], io: CliIO, diag: DiagnosticsConfig): Promise<number> {
  const p = parseArgs(rest);
  const source = p._[0];
  if (!source) {
    io.errln("export-diagram: missing <source>");
    return 1;
  }
  const view = strFlag(p, "view") ?? "mission";
  if (view !== "mission") {
    io.errln(`export-diagram: unsupported view "${view}" (only "mission")`);
    return 1;
  }
  const src = await resolveSource(source);
  if (src.kind === "descriptor") {
    io.errln("export-diagram: descriptor input not yet supported");
    return 1;
  }
  const registry = makeRegistry(src, diag);
  const seed = strFlag(p, "seed") ?? src.seed ?? "cyclevania";
  const reachIdx = intFlag(p, "reach", 0);
  const { reaches } = realize(registry, src, { seed, reaches: reachIdx + 1 });
  const reach = reaches[reachIdx] ?? reaches[reaches.length - 1];
  if (!reach) {
    io.errln("export-diagram: no reach generated");
    return 1;
  }
  const mermaid = reachMissionDiagram(assembleReach(reach));
  const out = strFlag(p, "out");
  if (out) {
    writeFileSync(out, mermaid + "\n", "utf8");
    io.errln(`export-diagram: wrote ${out}`);
  } else {
    io.outln(mermaid);
  }
  return 0;
}
