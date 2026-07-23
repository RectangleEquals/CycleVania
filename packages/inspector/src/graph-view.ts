/** Shared SVG node-graph renderer used by the World and Mission views. */

import { svg, clear } from "./dom.js";

export interface GNode {
  id: string;
  label: string;
  sub?: string;
  role: string;
  x: number;
  y: number;
  badge?: string;
  ghost?: boolean;
}
export interface GEdge {
  from: string;
  to: string;
  label?: string;
  gated?: boolean;
}

export const ROLE_COLORS: Record<string, string> = {
  hub: "#2d4a6b",
  segment: "#2b3a2b",
  gate: "#5b3a2b",
  vault: "#3a2b4a",
  capstone: "#4a2b2b",
  terminal: "#20242b",
  junction: "#2e2e2e",
  reach: "#24435f",
  portal: "#3a3320",
  default: "#232a33",
};

export const NODE_W = 128;
export const NODE_H = 44;

export interface GraphOpts {
  selectedId?: string;
  onSelect?: (id: string) => void;
  onOpen?: (id: string) => void;
}

export function renderGraph(host: HTMLElement, nodes: GNode[], edges: GEdge[], opts: GraphOpts = {}): void {
  clear(host);
  if (nodes.length === 0) {
    host.append(svg("svg", { viewBox: "0 0 100 100" }));
    return;
  }
  const pad = 40;
  const minX = Math.min(...nodes.map((n) => n.x)) - pad;
  const minY = Math.min(...nodes.map((n) => n.y)) - pad;
  const maxX = Math.max(...nodes.map((n) => n.x + NODE_W)) + pad;
  const maxY = Math.max(...nodes.map((n) => n.y + NODE_H)) + pad;
  const root = svg("svg", { viewBox: `${minX} ${minY} ${maxX - minX} ${maxY - minY}`, preserveAspectRatio: "xMidYMid meet" });

  const defs = svg("defs");
  const marker = svg("marker", { id: "arrow", viewBox: "0 0 10 10", refX: "9", refY: "5", markerWidth: "6", markerHeight: "6", orient: "auto-start-reverse" });
  marker.append(svg("path", { d: "M 0 0 L 10 5 L 0 10 z", fill: "#8a97a8" }));
  defs.append(marker);
  root.append(defs);

  const center = new Map(nodes.map((n) => [n.id, { x: n.x + NODE_W / 2, y: n.y + NODE_H / 2 }] as const));

  for (const e of edges) {
    const a = center.get(e.from);
    const b = center.get(e.to);
    if (!a || !b) continue;
    const line = svg("line", { class: `edge${e.gated ? " gated" : ""}`, x1: a.x, y1: a.y, x2: b.x, y2: b.y });
    root.append(line);
    if (e.label) {
      const t = svg("text", { class: "edge-label", x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 - 3, "text-anchor": "middle" });
      t.textContent = e.label;
      root.append(t);
    }
  }

  for (const n of nodes) {
    const g = svg("g", { style: "cursor:pointer" });
    const rect = svg("rect", {
      class: `node-box${opts.selectedId === n.id ? " sel" : ""}`,
      x: n.x,
      y: n.y,
      width: NODE_W,
      height: NODE_H,
      rx: 7,
      fill: ROLE_COLORS[n.role] ?? ROLE_COLORS["default"]!,
      "fill-opacity": n.ghost ? "0.25" : "1",
      "stroke-dasharray": n.ghost ? "4 3" : "0",
    });
    g.append(rect);
    const label = svg("text", { x: n.x + NODE_W / 2, y: n.y + 18, "text-anchor": "middle", "font-weight": "bold" });
    label.textContent = n.label;
    g.append(label);
    if (n.sub) {
      const sub = svg("text", { x: n.x + NODE_W / 2, y: n.y + 33, "text-anchor": "middle", fill: "#8a97a8" });
      sub.textContent = n.sub + (n.badge ? `  ${n.badge}` : "");
      g.append(sub);
    } else if (n.badge) {
      const b = svg("text", { x: n.x + NODE_W / 2, y: n.y + 33, "text-anchor": "middle", fill: "#8a97a8" });
      b.textContent = n.badge;
      g.append(b);
    }
    g.addEventListener("click", () => opts.onSelect?.(n.id));
    g.addEventListener("dblclick", () => opts.onOpen?.(n.id));
    root.append(g);
  }

  host.append(root);
}
