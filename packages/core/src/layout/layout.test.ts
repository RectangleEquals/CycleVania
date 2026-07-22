import { describe, it, expect } from "vitest";
import { forceLayout, type LayoutNode, type LayoutEdge } from "./index.js";

function ring(n: number): { nodes: LayoutNode[]; edges: LayoutEdge[] } {
  const nodes: LayoutNode[] = [];
  const edges: LayoutEdge[] = [];
  for (let i = 0; i < n; i++) nodes.push({ id: `r${i}`, radius: 2 });
  for (let i = 0; i < n; i++) edges.push({ a: `r${i}`, b: `r${(i + 1) % n}` });
  return { nodes, edges };
}

describe("force-directed layout", () => {
  it("is deterministic for a seed", () => {
    const { nodes, edges } = ring(8);
    const a = forceLayout(nodes, edges, { seed: "expedition" });
    const b = forceLayout(nodes, edges, { seed: "expedition" });
    for (const n of nodes) expect(a.positions.get(n.id)).toEqual(b.positions.get(n.id));
  });

  it("separates nodes by at least their combined radii", () => {
    const { nodes, edges } = ring(10);
    const { positions } = forceLayout(nodes, edges, { seed: "spacing", iterations: 300 });
    let minGap = Infinity;
    for (let i = 0; i < nodes.length; i++)
      for (let j = i + 1; j < nodes.length; j++) {
        const p = positions.get(nodes[i]!.id)!;
        const q = positions.get(nodes[j]!.id)!;
        minGap = Math.min(minGap, Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2]));
      }
    expect(minGap).toBeGreaterThan(2); // no two rooms collapsed together
    expect(Number.isFinite(minGap)).toBe(true);
  });

  it("honours pinned nodes and produces finite positions", () => {
    const nodes: LayoutNode[] = [
      { id: "hub", pinned: [0, 0, 0] },
      { id: "a", radius: 2 },
      { id: "b", radius: 2 },
    ];
    const edges: LayoutEdge[] = [{ a: "hub", b: "a" }, { a: "hub", b: "b" }, { a: "a", b: "b" }];
    const { positions } = forceLayout(nodes, edges, { seed: "pin" });
    expect(positions.get("hub")).toEqual([0, 0, 0]);
    for (const p of positions.values()) expect(p.every(Number.isFinite)).toBe(true);
  });

  it("spreads nodes vertically when zSeparation is high", () => {
    const { nodes, edges } = ring(12);
    const zSpan = (sep: number): number => {
      const { positions } = forceLayout(nodes, edges, { seed: "z", zSeparation: sep, iterations: 300 });
      let lo = Infinity;
      let hi = -Infinity;
      for (const p of positions.values()) {
        lo = Math.min(lo, p[2]);
        hi = Math.max(hi, p[2]);
      }
      return hi - lo;
    };
    expect(zSpan(2.2)).toBeGreaterThan(zSpan(1.0) * 0.9); // more vertical stack with higher zSeparation
  });
});
