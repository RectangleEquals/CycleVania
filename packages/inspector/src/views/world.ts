/** World view — realized Reaches as nodes, ReachPortals as edges, next slot as a ghost. */

import { store } from "../state.js";
import { renderGraph, NODE_W, type GNode, type GEdge } from "../graph-view.js";

export function renderWorld(host: HTMLElement): void {
  const { reaches, world, selection } = store.state;
  const gap = NODE_W + 80;

  const nodes: GNode[] = reaches.map((r) => ({
    id: `reach-${r.meta.reachIndex}`,
    label: `Reach ${r.meta.reachIndex}`,
    sub: `${r.graph.regions.length} regions`,
    role: "reach",
    x: r.meta.reachIndex * gap,
    y: 0,
    badge: `ceil ${Math.round(r.meta.finalCeiling)}`,
  }));

  if (world && (world.drawnLength === undefined || reaches.length < world.drawnLength)) {
    try {
      const prev = world.previewReachEnvelope(reaches.length);
      nodes.push({
        id: "ghost",
        label: `Reach ${reaches.length}`,
        sub: `~${Math.round(prev.meanNoModifiers)} ceiling`,
        role: "reach",
        x: reaches.length * gap,
        y: 0,
        ghost: true,
        badge: prev.isDeclaredFinalReach ? "final" : "unrealized",
      });
    } catch {
      /* past end — no ghost */
    }
  }

  const edges: GEdge[] = world
    ? world.portals
        .filter((p) => reaches.some((r) => r.meta.reachIndex === p.toReach))
        .map((p) => ({ from: `reach-${p.fromReach}`, to: `reach-${p.toReach}`, ...(p.oneWay ? { label: "one-way" } : {}) }))
    : [];

  const selectedId = selection.kind === "reach" ? `reach-${selection.reachIndex}` : undefined;
  renderGraph(host, nodes, edges, {
    ...(selectedId ? { selectedId } : {}),
    onSelect: (id) => {
      const m = /reach-(\d+)/.exec(id);
      if (m) store.select({ kind: "reach", reachIndex: Number(m[1]) });
    },
    onOpen: (id) => {
      const m = /reach-(\d+)/.exec(id);
      if (m) {
        store.state.selection = { kind: "reach", reachIndex: Number(m[1]) };
        store.set({ view: "mission" });
      }
    },
  });
}
