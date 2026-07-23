/** Deterministic math foundation — the whole surface stands on this. */

export { Rng, fnv1a, shuffle } from "./rng.js";
export type { WeightedEntry } from "./rng.js";
export { dsin, dcos, datan, datan2, reduce, yawFromDirection, yawBasis } from "./trig.js";
export {
  add,
  sub,
  scale,
  mul,
  dot,
  cross,
  length,
  normalize,
  lerp3,
  vecEq,
  ZERO3,
} from "./vec.js";
export type { Vec3, WorldBox } from "./vec.js";
export { clamp, clamp01, lerp, invLerp, smoothstep } from "./curve.js";
export {
  boxFromCenterHalf,
  boxCenter,
  boxSize,
  boxOverlap,
  boxContainsPoint,
  boxUnion,
  boxUnionAll,
} from "./geom.js";
export { gradNoise3, fbm3, noise3, fbm3v } from "./noise.js";
export { solveQEF } from "./qef.js";
export { quantizeNormal, isQuantized } from "./quantize.js";
