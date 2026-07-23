/** Mission view — the selected Reach's region graph, laid out in Sphere columns. */

import { ruleSummary } from "@cyclevania/core";
import type { ReachResult } from "@cyclevania/core";
import { store } from "../state.js";
import { renderGraph, NODE_W, NODE_H, type GNode, type GEdge } from "../graph-view.js";

export function activeReach(): ReachResult | undefined {
  const { reaches, selection } = store.state;
  if (selection.kind !== "none" && "reachIndex" in selection) {
    const found = reaches.find((r) => r.meta.reachIndex === selection.reachIndex);
    if (found) return found;
  }
  return reaches[0];
}

export function renderMission(host: HTMLElement): void {
  const reach = activeReach();
  if (!reach) {
    renderGraph(host, [], []);
    return;
  }
  const { sphereStep, selection } = store.state;

  const sphereOf = new Map<string, number>();
  reach.meta.spheres.forEach((ids, i) => ids.forEach((id) => sphereOf.set(id, i)));
  const maxSphere = reach.meta.spheres.length;

  const colGap = NODE_W + 90;
  const rowGap = NODE_H + 26;
  const colCount = new Map<number, number>();

  const nodes: GNode[] = reach.graph.regions.map((rg) => {
    const s = sphereOf.get(rg.id) ?? maxSphere;
    const row = colCount.get(s) ?? 0;
    colCount.set(s, row + 1);
    const dimmed = sphereStep >= 0 && s > sphereStep;
    return {
      id: rg.id,
      label: rg.id,
      sub: rg.role,
      role: rg.role,
      x: s * colGap,
      y: row * rowGap,
      badge: `s${s === maxSphere ? "?" : s}`,
      ...(dimmed ? { ghost: true } : {}),
    };
  });

  const edges: GEdge[] = reach.graph.edges.map((e) => {
    const label = ruleSummary(e.rule);
    return { from: e.from, to: e.to, gated: e.rule.k !== "always", ...(label ? { label } : {}), ...(e.oneWay ? { label: label ? `${label} · one-way` : "one-way" } : {}) };
  });

  const selectedId = selection.kind === "region" ? selection.regionId : undefined;
  renderGraph(host, nodes, edges, {
    ...(selectedId ? { selectedId } : {}),
    onSelect: (id) => store.select({ kind: "region", reachIndex: reach.meta.reachIndex, regionId: id }),
    onOpen: (id) => store.select({ kind: "region", reachIndex: reach.meta.reachIndex, regionId: id }),
  });
}
