/**
 * Seeded, forkable RNG (sfc32 core, FNV-1a seeding).
 *
 * The determinism contract: no `Math.random`, no host trig for generation state
 * — every random draw flows through an `Rng` handed down from the world seed, and
 * subsystems isolate their streams with `fork(label)` so adding a draw in one
 * system never perturbs another. Same seed ⇒ identical results on every JS engine.
 *
 * NOTE: the sfc32 core, FNV-1a string seeding, splitmix32 expansion, 8-iteration
 * warmup, and non-advancing fork are a bit-identical port of the reference
 * implementation; golden-vector parity tests pin them (see math/golden). Do not
 * "optimize" the arithmetic — every operation is load-bearing for cross-engine
 * determinism, and `next`/`int`/`range`/`chance`/`pick`/`fork`/`fnv1a` are the
 * exact surface the golden vectors exercise (their signatures are frozen).
 *
 * `int(min, max)` is INCLUSIVE of both ends (the golden-pinned signature). The
 * `weighted`, `triangular`, and `shuffle` METHODS are additive conveniences the
 * golden record never exercises, so they do not perturb any pinned sequence.
 */

/** FNV-1a 32-bit hash of a string, with an optional basis for chaining. */
export function fnv1a(str: string, basis = 0x811c9dc5): number {
  let h = basis >>> 0;
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

export interface WeightedEntry<T> {
  item: T;
  weight: number;
}

export class Rng {
  private a: number;
  private b: number;
  private c: number;
  private d: number;

  constructor(seed: number | string) {
    const s = typeof seed === "string" ? fnv1a(seed) : seed >>> 0;
    // splitmix32 to expand one word into four well-mixed state words
    let x = s >>> 0;
    const split = (): number => {
      x = (x + 0x9e3779b9) >>> 0;
      let z = x;
      z = Math.imul(z ^ (z >>> 16), 0x21f0aaad);
      z = Math.imul(z ^ (z >>> 15), 0x735a2d97);
      return (z ^ (z >>> 15)) >>> 0;
    };
    this.a = split();
    this.b = split();
    this.c = split();
    this.d = split();
    // warm up
    for (let i = 0; i < 8; i++) this.next();
  }

  /** Uniform float in [0, 1). */
  next(): number {
    // sfc32
    const t = (((this.a + this.b) >>> 0) + this.d) >>> 0;
    this.d = (this.d + 1) >>> 0;
    this.a = this.b ^ (this.b >>> 9);
    this.b = (this.c + (this.c << 3)) >>> 0;
    this.c = ((this.c << 21) | (this.c >>> 11)) >>> 0;
    this.c = (this.c + t) >>> 0;
    return t / 4294967296;
  }

  /** Uniform integer in [min, max] INCLUSIVE. */
  int(min: number, max: number): number {
    return min + Math.floor(this.next() * (max - min + 1));
  }

  /** Uniform float in [min, max). */
  range(min: number, max: number): number {
    return min + this.next() * (max - min);
  }

  /** True with probability p. */
  chance(p: number): boolean {
    return this.next() < p;
  }

  /** Uniform pick from a non-empty array. */
  pick<T>(arr: readonly T[]): T {
    const v = arr[this.int(0, arr.length - 1)];
    if (v === undefined && arr.length === 0) throw new Error("Rng.pick on empty array");
    return v as T;
  }

  /**
   * Symmetric triangular draw over [min, max) — the mean of two uniforms, so
   * central values are likelier than the extremes. Consumes two draws. Used by
   * the complexity-formula entropy jitter.
   */
  triangular(min: number, max: number): number {
    const t = (this.next() + this.next()) * 0.5;
    return min + t * (max - min);
  }

  /**
   * Weighted pick from a non-empty entry list. Negative weights are treated as
   * zero. If every weight is ≤ 0, falls back to a uniform pick over the items
   * (still deterministic). Draws by cumulative weight over the live list so the
   * result never depends on array identity.
   */
  weighted<T>(entries: readonly WeightedEntry<T>[]): T {
    let total = 0;
    let last: T | undefined;
    let any = false;
    for (const e of entries) {
      total += e.weight > 0 ? e.weight : 0;
      last = e.item;
      any = true;
    }
    if (!any) throw new Error("Rng.weighted on empty entries");
    if (total <= 0) {
      const idx = this.int(0, entries.length - 1);
      let i = 0;
      for (const e of entries) {
        if (i === idx) return e.item;
        i++;
      }
      return last as T;
    }
    let r = this.next() * total;
    for (const e of entries) {
      const w = e.weight > 0 ? e.weight : 0;
      if (r < w) return e.item;
      r -= w;
    }
    return last as T; // floating-point edge guard: return the final entry
  }

  /** Fisher–Yates shuffle returning a NEW array (leaves the input untouched). */
  shuffle<T>(arr: readonly T[]): T[] {
    const a = arr.slice();
    for (let i = a.length - 1; i > 0; i--) {
      const j = Math.floor(this.next() * (i + 1));
      const t = a[i] as T;
      a[i] = a[j] as T;
      a[j] = t;
    }
    return a;
  }

  /**
   * Derive an independent child stream. Does NOT advance this stream, so
   * fork order is stable regardless of how much either stream is used.
   */
  fork(label: string): Rng {
    const mix = fnv1a(label, this.a ^ 0x811c9dc5) ^ this.b ^ Math.imul(this.c, 0x9e3779b1) ^ this.d;
    return new Rng(mix >>> 0);
  }
}

/** In-place Fisher–Yates shuffle using an `Rng`. Returns the same array. */
export function shuffle<T>(a: T[], rng: Rng): T[] {
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(rng.next() * (i + 1));
    const t = a[i] as T;
    a[i] = a[j] as T;
    a[j] = t;
  }
  return a;
}
