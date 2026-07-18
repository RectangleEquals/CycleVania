import { describe, it, expect } from "vitest";
import { Rng, fnv1a, shuffle } from "./rng.js";

describe("Rng", () => {
  it("is deterministic: same seed → same sequence", () => {
    const a = new Rng("seed");
    const b = new Rng("seed");
    const sa = Array.from({ length: 32 }, () => a.next());
    const sb = Array.from({ length: 32 }, () => b.next());
    expect(sa).toEqual(sb);
  });

  it("string and numeric seeds diverge across streams", () => {
    const a = Array.from({ length: 16 }, (_, i) => i).map(() => new Rng("alpha").next());
    const b = new Rng("bravo");
    expect(a[0]).not.toBe(b.next());
  });

  it("next() stays in [0, 1)", () => {
    const r = new Rng(7);
    for (let i = 0; i < 10000; i++) {
      const v = r.next();
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThan(1);
    }
  });

  it("int() is inclusive on both ends and in range", () => {
    const r = new Rng("ints");
    let sawMin = false;
    let sawMax = false;
    for (let i = 0; i < 5000; i++) {
      const v = r.int(3, 7);
      expect(v).toBeGreaterThanOrEqual(3);
      expect(v).toBeLessThanOrEqual(7);
      expect(Number.isInteger(v)).toBe(true);
      if (v === 3) sawMin = true;
      if (v === 7) sawMax = true;
    }
    expect(sawMin && sawMax).toBe(true);
  });

  it("fork does NOT advance the parent stream", () => {
    const r = new Rng("x");
    const x1 = r.next();
    r.fork("child"); // must not perturb r
    const x2 = r.next();

    const ref = new Rng("x");
    expect(ref.next()).toBe(x1);
    expect(ref.next()).toBe(x2);
  });

  it("forked streams are independent and stable by label", () => {
    const r1 = new Rng("world");
    const r2 = new Rng("world");
    expect(r1.fork("a").next()).toBe(r2.fork("a").next());
    expect(r1.fork("a").next()).not.toBe(r2.fork("b").next());
  });

  it("pick throws on empty array", () => {
    const r = new Rng(1);
    expect(() => r.pick([])).toThrow();
  });

  it("shuffle is a deterministic permutation", () => {
    const base = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    const s1 = shuffle(base.slice(), new Rng("s"));
    const s2 = shuffle(base.slice(), new Rng("s"));
    expect(s1).toEqual(s2);
    expect([...s1].sort((a, b) => a - b)).toEqual(base);
  });
});

describe("fnv1a", () => {
  it("is stable and unsigned 32-bit", () => {
    const h = fnv1a("hello");
    expect(h).toBe(fnv1a("hello"));
    expect(h).toBeGreaterThanOrEqual(0);
    expect(h).toBeLessThanOrEqual(0xffffffff);
    expect(fnv1a("hello")).not.toBe(fnv1a("hellp"));
  });
});
