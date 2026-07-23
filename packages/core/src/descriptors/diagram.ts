/**
 * Mission-graph → Mermaid, from descriptors only (pure string function, core-legal).
 * Both the CLI (`export-diagram`, `report`) and the inspector's Mission view render
 * the SAME source produced here, so a diagram never drifts between tools. Regions are
 * colored by role via `classDef`; edges are labeled with a compact rule summary; each
 * Region can carry its Sphere index as a badge.
 */

import type { Rule } from "../logic/index.js";
import type { MissionGraphData, ReachDescriptor } from "./shapes.js";

export interface DiagramOptions {
  title?: string;
  direction?: "TD" | "LR";
  /** Sphere partition (region ids per sphere) — badges each Region with its index. */
  spheres?: string[][];
}

/** A compact, human-readable one-line summary of an access Rule (for edge labels). */
export function ruleSummary(r: Rule): string {
  switch (r.k) {
    case "always":
      return "";
    case "have":
      return r.cap;
    case "count":
      return `${r.n}× ${r.cap}`;
    case "flag":
      return `⚑ ${r.name}`;
    case "not":
      return `¬(${ruleSummary(r.of)})`;
    case "and":
      return r.of.map(ruleSummary).filter(Boolean).join(" ∧ ");
    case "or":
      return r.of.map(ruleSummary).filter(Boolean).join(" ∨ ");
  }
}

const ROLE_STYLES: Record<string, string> = {
  hub: "fill:#2d4a6b,stroke:#6fa8dc,color:#fff",
  segment: "fill:#2b3a2b,stroke:#93c47d,color:#fff",
  gate: "fill:#5b3a2b,stroke:#e69138,color:#fff",
  vault: "fill:#3a2b4a,stroke:#c27ba0,color:#fff",
  capstone: "fill:#4a2b2b,stroke:#e06666,color:#fff",
  terminal: "fill:#111,stroke:#eee,color:#fff",
  junction: "fill:#333,stroke:#999,color:#ddd",
};

const sanitize = (label: string): string => label.replace(/"/g, "'").replace(/[\n\r]/g, " ");

/** Turn a serialized MissionGraph into a Mermaid `flowchart` source string. */
export function missionGraphMermaid(graph: MissionGraphData, opts: DiagramOptions = {}): string {
  const dir = opts.direction ?? "TD";
  const sphereOf = new Map<string, number>();
  if (opts.spheres) opts.spheres.forEach((ids, i) => ids.forEach((id) => sphereOf.set(id, i)));

  // stable, mermaid-safe node ids
  const nodeId = new Map<string, string>();
  graph.regions.forEach((r, i) => nodeId.set(r.id, `n${i}`));
  const idOf = (regionId: string): string => nodeId.get(regionId) ?? `n_${sanitize(regionId)}`;

  const lines: string[] = [];
  if (opts.title) lines.push(`---`, `title: ${sanitize(opts.title)}`, `---`);
  lines.push(`flowchart ${dir}`);

  const usedRoles = new Set<string>();
  for (const r of graph.regions) {
    const badge = sphereOf.has(r.id) ? ` [s${sphereOf.get(r.id)}]` : "";
    const star = r.id === graph.start ? " ▶" : "";
    const role = ROLE_STYLES[r.role] ? r.role : "junction";
    usedRoles.add(role);
    lines.push(`  ${idOf(r.id)}["${sanitize(r.id)}${star}\\n${sanitize(r.role)}${badge}"]:::${role}`);
  }

  for (const e of graph.edges) {
    const label = sanitize(ruleSummary(e.rule));
    const prefix = e.oneWay ? "one-way" : "";
    const text = [prefix, label].filter(Boolean).join(" · ");
    lines.push(text ? `  ${idOf(e.from)} -->|"${text}"| ${idOf(e.to)}` : `  ${idOf(e.from)} --> ${idOf(e.to)}`);
  }

  // gated Locations shown as leaf notes so a reader sees where keys live
  for (const loc of graph.locations) {
    const gate = loc.gate ? sanitize(ruleSummary(loc.gate)) : "";
    const bonus = loc.bonus ? " (bonus)" : "";
    if (gate || bonus) lines.push(`  ${idOf(loc.region)} -.-|"${[gate, bonus].filter(Boolean).join(" ")}"| loc_${sanitize(loc.id)}(("${sanitize(loc.id)}"))`);
  }

  for (const role of usedRoles) lines.push(`  classDef ${role} ${ROLE_STYLES[role]}`);
  return lines.join("\n");
}

/** Convenience: a Reach's mission graph, badged with its own sphere partition. */
export function reachMissionDiagram(reach: ReachDescriptor, opts: DiagramOptions = {}): string {
  return missionGraphMermaid(reach.graph, { spheres: reach.meta.spheres, title: `Reach ${reach.meta.reachIndex}`, ...opts });
}
