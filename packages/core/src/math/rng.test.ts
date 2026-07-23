import { describe, it, expect } from "vitest";
import { Rng, fnv1a } from "./rng.js";

describe("Rng", () => {
  it("is deterministic for a given seed", () => {
    const a = new Rng("seed-x");
    const b = new Rng("seed-x");
    const seqA = Array.from({ length: 20 }, () => a.next());
    const seqB = Array.from({ length: 20 }, () => b.next());
    expect(seqA).toEqual(seqB);
  });

  it("produces different streams for different seeds", () => {
    const a = new Rng("seed-x");
    const b = new Rng("seed-y");
    expect(a.next()).not.toBe(b.next());
  });

  it("fork does not advance the parent stream", () => {
    // Control: parent's sequence with no fork in between.
    const control = new Rng("p");
    const controlSeq = Array.from({ length: 10 }, () => control.next());

    // Same parent, but fork between draws: the parent sequence must be unchanged.
    const parent = new Rng("p");
    const forkedSeq: number[] = [];
    for (let i = 0; i < 10; i++) {
      parent.fork(`child-${i}`); // should not perturb `parent`
      forkedSeq.push(parent.next());
    }
    expect(forkedSeq).toEqual(controlSeq);
  });

  it("fork with the same label yields identical child streams", () => {
    const p1 = new Rng("root");
    const p2 = new Rng("root");
    const c1 = p1.fork("area:r2");
    const c2 = p2.fork("area:r2");
    expect(Array.from({ length: 8 }, () => c1.next())).toEqual(Array.from({ length: 8 }, () => c2.next()));
  });

  it("fork with different labels yields different child streams", () => {
    const p = new Rng("root");
    const c1 = p.fork("a");
    const c2 = p.fork("b");
    expect(c1.next()).not.toBe(c2.next());
  });

  it("int is inclusive of both ends and stays in range", () => {
    const rng = new Rng("ints");
    const seen = new Set<number>();
    for (let i = 0; i < 5000; i++) {
      const v = rng.int(3, 7);
      expect(v).toBeGreaterThanOrEqual(3);
      expect(v).toBeLessThanOrEqual(7);
      expect(Number.isInteger(v)).toBe(true);
      seen.add(v);
    }
    expect(seen).toEqual(new Set([3, 4, 5, 6, 7])); // both ends reachable
  });

  it("weighted respects weights over many draws", () => {
    const rng = new Rng("w");
    const counts = { a: 0, b: 0, c: 0 };
    for (let i = 0; i < 30000; i++) {
      const v = rng.weighted([
        { item: "a" as const, weight: 1 },
        { item: "b" as const, weight: 3 },
        { item: "c" as const, weight: 6 },
      ]);
      counts[v]++;
    }
    // Expected proportions 0.1 / 0.3 / 0.6; allow generous slack.
    expect(counts.a / 30000).toBeGreaterThan(0.06);
    expect(counts.a / 30000).toBeLessThan(0.14);
    expect(counts.c / 30000).toBeGreaterThan(0.54);
    expect(counts.c / 30000).toBeLessThan(0.66);
    expect(counts.b).toBeGreaterThan(counts.a);
    expect(counts.c).toBeGreaterThan(counts.b);
  });

  it("weighted falls back to uniform when all weights are zero", () => {
    const rng = new Rng("wz");
    const seen = new Set<string>();
    for (let i = 0; i < 200; i++) {
      seen.add(rng.weighted([
        { item: "a", weight: 0 },
        { item: "b", weight: 0 },
        { item: "c", weight: 0 },
      ]));
    }
    expect(seen).toEqual(new Set(["a", "b", "c"]));
  });

  it("shuffle returns a permutation, leaves the input untouched, and is deterministic", () => {
    const src = [1, 2, 3, 4, 5, 6, 7, 8];
    const r1 = new Rng("s");
    const r2 = new Rng("s");
    const out1 = r1.shuffle(src);
    const out2 = r2.shuffle(src);
    expect(out1).toEqual(out2); // deterministic
    expect(src).toEqual([1, 2, 3, 4, 5, 6, 7, 8]); // input untouched
    expect([...out1].sort((a, b) => a - b)).toEqual(src); // a permutation
  });

  it("triangular stays in bounds and centers around the midpoint", () => {
    const rng = new Rng("tri");
    let sum = 0;
    const N = 20000;
    for (let i = 0; i < N; i++) {
      const v = rng.triangular(-1, 1);
      expect(v).toBeGreaterThanOrEqual(-1);
      expect(v).toBeLessThan(1);
      sum += v;
    }
    expect(Math.abs(sum / N)).toBeLessThan(0.05); // mean ≈ 0
  });

  it("fnv1a is stable and basis-chainable", () => {
    expect(fnv1a("")).toBe(fnv1a(""));
    expect(fnv1a("hello")).not.toBe(fnv1a("world"));
    expect(fnv1a("child", 0x1234)).not.toBe(fnv1a("child"));
  });
});
