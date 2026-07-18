/**
 * Deterministic sin/cos/atan — polynomial minimax approximation with pure
 * arithmetic, giving identical results on every JS engine (unlike `Math.sin`).
 * Accuracy ~1e-6 over the full range after reduction; plenty for placement.
 *
 * Bit-identical port of the reference implementation. World/sim space is
 * left-handed Z-up (X east, Y north, Z up); yaw 0 faces +Y (north), CCW positive.
 * Golden-vector parity tests pin the coefficients (see math/golden).
 */

const TWO_PI = Math.PI * 2;
const INV_TWO_PI = 1 / TWO_PI;

/** Reduce to [-π, π] deterministically. */
export function reduce(a: number): number {
  const k = Math.floor(a * INV_TWO_PI + 0.5);
  return a - k * TWO_PI;
}

/**
 * sin on [-π, π] via odd minimax polynomial (degree 11).
 * Coefficients: standard fdlibm-style, exact double literals.
 */
function sinPoly(x: number): number {
  const x2 = x * x;
  return (
    x *
    (0.9999999999999999 +
      x2 *
        (-0.16666666666664811 +
          x2 *
            (0.008333333333226519 +
              x2 *
                (-0.00019841269813888534 +
                  x2 * (0.0000027557315514280769 + x2 * -0.000000025051823583393708)))))
  );
}

export function dsin(a: number): number {
  const x = reduce(a);
  // fold to [-π/2, π/2] where the polynomial is most accurate
  if (x > Math.PI / 2) return sinPoly(Math.PI - x);
  if (x < -Math.PI / 2) return sinPoly(-Math.PI - x);
  return sinPoly(x);
}

export function dcos(a: number): number {
  return dsin(a + Math.PI / 2);
}

/**
 * Deterministic atan (minimax, ~2e-6) and atan2 — used to derive a facing yaw
 * from a direction vector.
 */
function atanPoly(x: number): number {
  // valid on [-1, 1]
  const x2 = x * x;
  return (
    x *
    (0.9999993329 +
      x2 *
        (-0.3332985605 +
          x2 * (0.1994653599 + x2 * (-0.1390853351 + x2 * (0.0964200441 + x2 * (-0.0559098861 + x2 * (0.0218612288 + x2 * -0.004054058)))))))
  );
}

export function datan(x: number): number {
  if (x > 1) return Math.PI / 2 - atanPoly(1 / x);
  if (x < -1) return -Math.PI / 2 - atanPoly(1 / x);
  return atanPoly(x);
}

export function datan2(y: number, x: number): number {
  if (x > 0) return datan(y / x);
  if (x < 0) return y >= 0 ? datan(y / x) + Math.PI : datan(y / x) - Math.PI;
  if (y > 0) return Math.PI / 2;
  if (y < 0) return -Math.PI / 2;
  return 0;
}

/** Sim yaw (0 = +Y north, CCW+) that faces along direction (dx, dy). */
export function yawFromDirection(dx: number, dy: number): number {
  // forward(yaw) = (−sin yaw, cos yaw) ⇒ yaw = atan2(−dx, dy)
  return datan2(-dx, dy);
}

/** Forward/right basis on the world XY plane from a yaw (0 = +Y north, CCW+). */
export function yawBasis(yaw: number): { fx: number; fy: number; rx: number; ry: number } {
  const s = dsin(yaw);
  const c = dcos(yaw);
  // forward rotates from +Y; right is forward rotated -90° about +Z
  return { fx: -s, fy: c, rx: c, ry: s };
}
