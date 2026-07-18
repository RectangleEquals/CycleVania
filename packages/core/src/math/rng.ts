/**
 * Seeded, forkable RNG (sfc32 core, FNV-1a seeding).
 *
 * The determinism contract: no `Math.random`, no host trig for generation state
 * — every random draw flows through an `Rng` handed down from the world seed, and
 * subsystems isolate their streams with `fork(label)` so adding a draw in one
 * system never perturbs another. Same seed ⇒ identical results on every JS engine.
 *
 * NOTE: This is a bit-identical port of the reference implementation (sfc32 core,
 * FNV-1a string seeding, splitmix32 expansion, 8-iteration warmup, non-advancing
 * fork). Golden-vector parity tests pin it (see math/golden). Do not "optimize"
 * the arithmetic — every operation is load-bearing for cross-engine determinism.
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

  /** Uniform integer in [min, max] inclusive. */
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
