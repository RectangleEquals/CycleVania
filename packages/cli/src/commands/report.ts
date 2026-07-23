import { writeFileSync } from "node:fs";
import { assembleReach, reachMissionDiagram } from "@cyclevania/core";
import type { DiagnosticsConfig, ReachResult } from "@cyclevania/core";
import { parseArgs, strFlag, boolFlag, intFlag } from "../args.js";
import type { CliIO } from "../io.js";
import { resolveSource } from "../sources.js";
import { makeRegistry, realize, type RealizeOptions } from "../pipeline.js";

function reachSection(r: ReachResult): string {
  const lines: string[] = [];
  lines.push(`## Reach ${r.meta.reachIndex}`, "");
  lines.push("```mermaid", reachMissionDiagram(assembleReach(r)), "```", "");

  lines.push("### Spheres", "", "| Sphere | Regions |", "|---|---|");
  r.meta.spheres.forEach((regions, i) => lines.push(`| ${i} | ${regions.join(", ")} |`));
  lines.push("");

  lines.push("### Placement", "", "| Location | Item |", "|---|---|");
  for (const [loc, item] of [...r.placement.entries()].sort((a, b) => (a[0] < b[0] ? -1 : 1))) lines.push(`| ${loc} | ${item} |`);
  lines.push("");

  lines.push("### Dials", "", `- final ceiling: ${r.meta.finalCeiling}`, `- areas: ${r.meta.areaCount}`, `- chosen modifiers: ${r.meta.chosenModifiers.join(", ") || "none"}`);
  const buckets = Object.entries(r.buckets).filter(([, v]) => v !== 0);
  if (buckets.length) lines.push(`- budget buckets: ${buckets.map(([k, v]) => `${k}=${v.toFixed(2)}`).join(", ")}`);
  lines.push("");

  if (r.meta.relaxations.length) {
    lines.push("### Relaxations", "");
    for (const rel of r.meta.relaxations) lines.push(`- ${rel}`);
    lines.push("");
  }

  const withGeo = r.skeleton.areas.filter((a) => a.finish);
  if (withGeo.length) {
    lines.push("### Geometry", "", "| Area | Tris | Unique pieces | Instances |", "|---|---|---|---|");
    for (const a of withGeo) lines.push(`| ${a.regionId} | ${a.finish!.stats.tris} | ${a.finish!.stats.uniquePieces} | ${a.finish!.instances.length} |`);
    lines.push("");
  }
  return lines.join("\n");
}

/** report <source> --seed S -o report.md [--geometry] [--reaches N] */
export async function run(rest: string[], io: CliIO, diag: DiagnosticsConfig): Promise<number> {
  const p = parseArgs(rest, ["geometry"]);
  const source = p._[0];
  if (!source) {
    io.errln("report: missing <source>");
    return 1;
  }
  const src = await resolveSource(source);
  if (src.kind === "descriptor") {
    io.errln("report: descriptor input not yet supported (pass a preset/bundle to regenerate)");
    return 1;
  }
  const registry = makeRegistry(src, diag);
  const seed = strFlag(p, "seed") ?? src.seed ?? "cyclevania";
  const ropts: RealizeOptions = { seed, reaches: strFlag(p, "reaches") !== undefined ? Math.max(1, intFlag(p, "reaches", 1)) : 1 };
  if (boolFlag(p, "geometry")) ropts.geometry = true;
  const { reaches } = realize(registry, src, ropts);

  const md: string[] = [];
  md.push(`# CycleVania world report`, "");
  md.push(`- Seed: \`${seed}\``);
  md.push(`- Dataset: \`${src.ref ? `${src.ref.module}#${src.ref.export}` : source}\``);
  md.push(`- Fingerprint: \`${registry.fingerprint}\``);
  md.push(`- Reaches: ${reaches.length}`, "");
  for (const r of reaches) md.push(reachSection(r));

  const text = md.join("\n");
  const out = strFlag(p, "out");
  if (out) {
    writeFileSync(out, text, "utf8");
    io.errln(`report: wrote ${out}`);
  } else {
    io.outln(text);
  }
  return 0;
}
