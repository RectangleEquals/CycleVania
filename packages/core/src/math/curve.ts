/**
 * Scalar curve helpers used by the complexity budget and difficulty scaling.
 * All pure arithmetic (no host trig) so they stay determinism-safe.
 */

export const clamp = (x: number, lo: number, hi: number): number => (x < lo ? lo : x > hi ? hi : x);

export const clamp01 = (x: number): number => clamp(x, 0, 1);

export const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;

/** Inverse lerp: where does `x` sit in [a, b]? (unclamped) */
export const invLerp = (a: number, b: number, x: number): number => (b === a ? 0 : (x - a) / (b - a));

/** Smoothstep S-curve on [0, 1] — gentle at both ends. */
export const smoothstep = (t: number): number => {
  const x = clamp01(t);
  return x * x * (3 - 2 * x);
};
