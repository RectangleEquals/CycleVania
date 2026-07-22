/**
 * Seeded, deterministic 3D gradient (Perlin-style) noise + fbm. Pure integer
 * hashing + arithmetic — no `Math.random`, no host trig — so it's identical on
 * every engine (used to warp SDF hulls into organic shapes).
 */

const GRAD3: ReadonlyArray<readonly [number, number, number]> = [
  [1, 1, 0], [-1, 1, 0], [1, -1, 0], [-1, -1, 0],
  [1, 0, 1], [-1, 0, 1], [1, 0, -1], [-1, 0, -1],
  [0, 1, 1], [0, -1, 1], [0, 1, -1], [0, -1, -1],
];

function hashLattice(ix: number, iy: number, iz: number, seed: number): number {
  let h = seed >>> 0;
  h = Math.imul(h ^ (ix | 0), 0x27d4eb2f) >>> 0;
  h = Math.imul(h ^ (iy | 0), 0x85ebca6b) >>> 0;
  h = Math.imul(h ^ (iz | 0), 0xc2b2ae35) >>> 0;
  h ^= h >>> 15;
  return h >>> 0;
}

function grad(ix: number, iy: number, iz: number, seed: number, dx: number, dy: number, dz: number): number {
  const g = GRAD3[hashLattice(ix, iy, iz, seed) % 12] as readonly [number, number, number];
  return g[0] * dx + g[1] * dy + g[2] * dz;
}

const fade = (t: number): number => t * t * t * (t * (t * 6 - 15) + 10);
const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;

/** Gradient noise in ~[-1, 1]. */
export function gradNoise3(x: number, y: number, z: number, seed = 0): number {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const zi = Math.floor(z);
  const xf = x - xi;
  const yf = y - yi;
  const zf = z - zi;
  const u = fade(xf);
  const v = fade(yf);
  const w = fade(zf);
  const n000 = grad(xi, yi, zi, seed, xf, yf, zf);
  const n100 = grad(xi + 1, yi, zi, seed, xf - 1, yf, zf);
  const n010 = grad(xi, yi + 1, zi, seed, xf, yf - 1, zf);
  const n110 = grad(xi + 1, yi + 1, zi, seed, xf - 1, yf - 1, zf);
  const n001 = grad(xi, yi, zi + 1, seed, xf, yf, zf - 1);
  const n101 = grad(xi + 1, yi, zi + 1, seed, xf - 1, yf, zf - 1);
  const n011 = grad(xi, yi + 1, zi + 1, seed, xf, yf - 1, zf - 1);
  const n111 = grad(xi + 1, yi + 1, zi + 1, seed, xf - 1, yf - 1, zf - 1);
  const x00 = lerp(n000, n100, u);
  const x10 = lerp(n010, n110, u);
  const x01 = lerp(n001, n101, u);
  const x11 = lerp(n011, n111, u);
  return lerp(lerp(x00, x10, v), lerp(x01, x11, v), w);
}

/** Fractal Brownian motion (summed octaves) in ~[-1, 1]. */
export function fbm3(x: number, y: number, z: number, seed = 0, octaves = 4, lacunarity = 2, gain = 0.5): number {
  let amp = 0.5;
  let freq = 1;
  let sum = 0;
  let norm = 0;
  for (let o = 0; o < octaves; o++) {
    sum += amp * gradNoise3(x * freq, y * freq, z * freq, (seed + o * 131) >>> 0);
    norm += amp;
    amp *= gain;
    freq *= lacunarity;
  }
  return norm > 0 ? sum / norm : 0;
}
