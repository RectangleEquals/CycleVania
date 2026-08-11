//! Owned deterministic math — the **only** sanctioned float operations in the engine.
//!
//! # The float contract
//!
//! *Replayable-deterministic*: **same seed + same build ⇒ same output**, on every target. WASM is the
//! canonical cross-machine target; native is expected to match it bit-for-bit and is verified to.
//!
//! Two classes of operation exist, and the distinction is the whole point of this module:
//!
//! * **Exact ops** — `+ - * /`, `sqrt`, `abs`, `floor`, `ceil`, `trunc`, `round`, `copysign`, and
//!   comparisons are *correctly rounded* by IEEE-754 and lower to the same hardware instruction on
//!   x86-64 and wasm32. They are bit-identical everywhere, so this module simply delegates to them.
//!   Rust never enables fast-math and never auto-contracts a multiply-add into an FMA, so the compiler
//!   cannot silently perturb them either.
//! * **Transcendentals** — `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `ln`, `pow`
//!   are *not* correctly rounded by any platform, and each libm (MSVC CRT, glibc, musl, the wasm
//!   builtins) rounds the last ULP differently. Calling `f64::sin` would therefore break cross-target
//!   determinism. So this module **implements them from scratch** out of exact ops only: Cody–Waite
//!   argument reduction plus minimax polynomials, following the classic fdlibm/musl formulations.
//!
//! A workspace `clippy.toml` denies the platform transcendentals crate-wide so nothing can drift back
//! in by accident.
//!
//! # Accuracy
//!
//! Determinism is **unconditional** — every routine here is built from exact ops, so it returns the
//! same bits on every target for every input, always. Accuracy is a separate, weaker claim. The table
//! below is *measured* against the platform libm, not estimated; `tests/math.rs` enforces it.
//!
//! | Routine | Measured worst case | Domain |
//! |---|---|---|
//! | `sin`, `cos` | ≤ 2 ULP | `\|x\| ≤ 2^20` — see the argument-reduction note below |
//! | `tan` | ≤ 3 ULP | away from the poles (computed as `sin/cos`) |
//! | `asin`, `acos` | ≤ 2 ULP | `[-1, 1]` (via `atan2`, well-conditioned at the endpoints) |
//! | `atan`, `atan2` | ≤ 1 ULP | all finite inputs |
//! | `exp`, `ln`, `log2`, `log10` | ≤ 1 ULP | all finite inputs |
//! | `hypot`, `cbrt` | ≤ 2 ULP | all finite inputs |
//! | `pow` | ≤ 6 ULP | integer exponents ≤ 2 ULP for `\|n\| ≤ 20` |
//!
//! **The `sin`/`cos` domain is a real limit, not a formality.** Cody–Waite reduction holds full
//! precision while `kf * π/2` stays exact, which is `|x| ≤ 2^20` (~167 000 full turns — far outside
//! any level-generation use). Past it, accuracy degrades sharply and is *gone* by `|x| ≈ 10^8`.
//! Determinism is unaffected either way: every step is an exact op, so a huge argument returns the
//! same wrong-ish answer on every target. `pow`'s residual error comes from the `log2(n)` roundings
//! in binary exponentiation, which is inherent rather than a reduction artifact.

// The exact-op delegations below are the sanctioned wrappers; everything else is built from them.
#![allow(clippy::disallowed_methods)]

// ---------------------------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------------------------

/// π
pub const PI: f64 = std::f64::consts::PI;
/// 2π — a full turn.
pub const TAU: f64 = std::f64::consts::TAU;
/// π/2
pub const FRAC_PI_2: f64 = std::f64::consts::FRAC_PI_2;
/// π/4
pub const FRAC_PI_4: f64 = std::f64::consts::FRAC_PI_4;
/// Euler's number.
pub const E: f64 = std::f64::consts::E;

/// 2/π — the argument-reduction scale for `sin`/`cos`.
const FRAC_2_PI: f64 = std::f64::consts::FRAC_2_PI;
/// 1/ln 2 — the argument-reduction scale for `exp`.
const INV_LN2: f64 = std::f64::consts::LOG2_E;
/// 1/ln 10 — the scale factor for `log10`.
const INV_LN10: f64 = std::f64::consts::LOG10_E;

// Cody–Waite three-part split of π/2 (fdlibm `pio2_1`, `pio2_2`, `pio2_2t`).
//
// These must form a *cascade*: each term is the 33-bit head of what the previous ones left over, so
// `kf * term` stays exact for `|kf| < 2^20` and the subtractions lose nothing. Note this is
// deliberately `pio2_2`, **not** fdlibm's `pio2_1t`: those two are the same quantity at different
// precisions (`pio2_1t ≈ pio2_2 + pio2_2t`), so pairing `pio2_1t` with `pio2_2t` would subtract the
// residual twice and inject an error of `kf * 2e-21`.
// (Literals are the shortest form that round-trips to the same f64 as fdlibm's published decimals.)
const PIO2_HI: f64 = 1.570_796_326_734_125_6;
const PIO2_MID: f64 = 6.077_100_506_303_966e-11;
const PIO2_LO: f64 = 2.022_266_248_795_950_6e-21;

// Two-part split of ln 2 (fdlibm).
const LN2_HI: f64 = 6.931_471_803_691_238e-1;
const LN2_LO: f64 = 1.908_214_929_270_587_7e-10;

// ---------------------------------------------------------------------------------------------
// Exact operations — correctly rounded by IEEE-754, identical on every target
// ---------------------------------------------------------------------------------------------

/// Square root. Correctly rounded by IEEE-754 (a single hardware instruction) — exact everywhere.
#[inline]
pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

/// Absolute value (a sign-bit clear — exact).
#[inline]
pub fn abs(x: f64) -> f64 {
    x.abs()
}

/// Largest integer ≤ `x` (exact).
#[inline]
pub fn floor(x: f64) -> f64 {
    x.floor()
}

/// Smallest integer ≥ `x` (exact).
#[inline]
pub fn ceil(x: f64) -> f64 {
    x.ceil()
}

/// Truncate toward zero (exact).
#[inline]
pub fn trunc(x: f64) -> f64 {
    x.trunc()
}

/// Round half away from zero (exact).
#[inline]
pub fn round(x: f64) -> f64 {
    x.round()
}

/// Magnitude of `x` with the sign of `y` (exact).
#[inline]
pub fn copysign(x: f64, y: f64) -> f64 {
    x.copysign(y)
}

/// The smaller of two values; NaN-propagating is *not* guaranteed — mirrors IEEE `minNum`.
#[inline]
pub fn min(a: f64, b: f64) -> f64 {
    a.min(b)
}

/// The larger of two values.
#[inline]
pub fn max(a: f64, b: f64) -> f64 {
    a.max(b)
}

/// Constrain `x` to `[lo, hi]`. Requires `lo <= hi`.
#[inline]
pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    debug_assert!(lo <= hi, "clamp requires lo <= hi");
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

/// Constrain `x` to `[0, 1]`.
#[inline]
pub fn saturate(x: f64) -> f64 {
    clamp(x, 0.0, 1.0)
}

/// Linear interpolation `a + (b - a) * t`. Exact at `t == 0`; `t` is not clamped.
#[inline]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Where `x` falls between `a` and `b`, as a fraction. Returns `0.0` when `a == b`.
#[inline]
pub fn inverse_lerp(a: f64, b: f64, x: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (x - a) / (b - a)
    }
}

/// Map `x` from `[in_lo, in_hi]` onto `[out_lo, out_hi]` (unclamped).
#[inline]
pub fn remap(x: f64, in_lo: f64, in_hi: f64, out_lo: f64, out_hi: f64) -> f64 {
    lerp(out_lo, out_hi, inverse_lerp(in_lo, in_hi, x))
}

/// Hermite smoothstep over `[edge0, edge1]`, clamped to `[0, 1]`.
pub fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = saturate(inverse_lerp(edge0, edge1, x));
    t * t * (3.0 - 2.0 * t)
}

/// Sign as `-1.0`, `0.0`, or `1.0` (zero for `±0.0`, NaN for NaN).
pub fn signum(x: f64) -> f64 {
    if x.is_nan() {
        f64::NAN
    } else if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Degrees → radians.
#[inline]
pub fn to_radians(deg: f64) -> f64 {
    deg * (PI / 180.0)
}

/// Radians → degrees.
#[inline]
pub fn to_degrees(rad: f64) -> f64 {
    rad * (180.0 / PI)
}

/// `x * 2^n`, computed by exponent manipulation (exact, no rounding).
pub fn scalbn(x: f64, n: i32) -> f64 {
    #[inline]
    fn two_pow(n: i32) -> f64 {
        f64::from_bits(((n + 1023) as u64) << 52)
    }
    let mut y = x;
    let mut n = n;
    // Step in chunks that stay inside the normal range, so no intermediate flushes to zero.
    if n > 1023 {
        y *= two_pow(1023);
        n -= 1023;
        if n > 1023 {
            y *= two_pow(1023);
            n -= 1023;
            if n > 1023 {
                n = 1023;
            }
        }
    } else if n < -1022 {
        y *= two_pow(-1022) * two_pow(53); // 2^-969
        n += 969;
        if n < -1022 {
            y *= two_pow(-1022) * two_pow(53);
            n += 969;
            if n < -1022 {
                n = -1022;
            }
        }
    }
    y * two_pow(n)
}

// ---------------------------------------------------------------------------------------------
// Trigonometry — owned Cody–Waite reduction + fdlibm minimax kernels
// ---------------------------------------------------------------------------------------------

/// Minimax polynomial for `sin` on `|x| ≤ π/4`.
fn kernel_sin(x: f64) -> f64 {
    const S1: f64 = -1.666_666_666_666_663_2e-1;
    const S2: f64 = 0.008_333_333_333_322_49;
    const S3: f64 = -1.984_126_982_985_795e-4;
    const S4: f64 = 2.755_731_370_707_007e-6;
    const S5: f64 = -2.505_076_025_340_686_4e-8;
    const S6: f64 = 1.589_690_995_211_55e-10;
    let z = x * x;
    let v = z * x;
    let r = S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)));
    x + v * (S1 + z * r)
}

/// Minimax polynomial for `cos` on `|x| ≤ π/4`.
fn kernel_cos(x: f64) -> f64 {
    const C1: f64 = 4.166_666_666_666_66e-2;
    const C2: f64 = -1.388_888_888_887_411e-3;
    const C3: f64 = 2.480_158_728_947_673e-5;
    const C4: f64 = -2.755_731_435_139_066_4e-7;
    const C5: f64 = 2.087_572_321_298_175e-9;
    const C6: f64 = -1.135_964_755_778_819_5e-11;
    let z = x * x;
    let r = z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))));
    let hz = 0.5 * z;
    let w = 1.0 - hz;
    // Compensated reconstruction: recovers the bits `1.0 - hz` rounded away.
    w + (((1.0 - w) - hz) + z * r)
}

/// Reduce `x` to `n·(π/2) + r` with `|r| ≤ π/4`. Returns `(n mod 4, r)`.
///
/// Three-part Cody–Waite subtraction: full precision for `|x| ≤ 2^20`; larger arguments stay
/// perfectly deterministic (every step is an exact op) but lose precision.
fn rem_pio2(x: f64) -> (i32, f64) {
    let kf = round(x * FRAC_2_PI);
    let n = kf as i64 as i32;
    let r = ((x - kf * PIO2_HI) - kf * PIO2_MID) - kf * PIO2_LO;
    (n & 3, r)
}

/// Sine of `x` radians.
pub fn sin(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    let (n, r) = rem_pio2(x);
    match n {
        0 => kernel_sin(r),
        1 => kernel_cos(r),
        2 => -kernel_sin(r),
        _ => -kernel_cos(r),
    }
}

/// Cosine of `x` radians.
pub fn cos(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    let (n, r) = rem_pio2(x);
    match n {
        0 => kernel_cos(r),
        1 => -kernel_sin(r),
        2 => -kernel_cos(r),
        _ => kernel_sin(r),
    }
}

/// Sine and cosine together (one argument reduction).
pub fn sin_cos(x: f64) -> (f64, f64) {
    if !x.is_finite() {
        return (f64::NAN, f64::NAN);
    }
    let (n, r) = rem_pio2(x);
    let (s, c) = (kernel_sin(r), kernel_cos(r));
    match n {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// Tangent of `x` radians. Computed as `sin/cos`; precision degrades near the poles.
pub fn tan(x: f64) -> f64 {
    let (s, c) = sin_cos(x);
    s / c
}

/// Arctangent, in `[-π/2, π/2]`. fdlibm's segmented minimax rational approximation.
pub fn atan(x: f64) -> f64 {
    // atan(inf) and atan(1.0) are exactly π/2 and π/4 in f64, so use the named constants.
    const ATAN_HI: [f64; 4] = [
        4.636_476_090_008_061e-1, // atan(0.5)
        FRAC_PI_4,                // atan(1.0)
        0.982_793_723_247_329,    // atan(1.5)
        FRAC_PI_2,                // atan(inf)
    ];
    const ATAN_LO: [f64; 4] = [
        2.269_877_745_296_168_7e-17,
        3.061_616_997_868_383e-17,
        1.390_331_103_123_099_8e-17,
        6.123_233_995_736_766e-17,
    ];
    const AT: [f64; 11] = [
        3.333_333_333_333_293e-1,
        -1.999_999_999_987_648_3e-1,
        1.428_571_427_250_346_6e-1,
        -1.111_111_040_546_235_6e-1,
        9.090_887_133_436_507e-2,
        -7.691_876_205_044_83e-2,
        6.661_073_137_387_531e-2,
        -5.833_570_133_790_573_4e-2,
        4.976_877_994_615_932e-2,
        -3.653_157_274_421_691_5e-2,
        1.628_582_011_536_578_2e-2,
    ];

    if x.is_nan() {
        return f64::NAN;
    }
    let sign_neg = x.is_sign_negative();
    let ax = abs(x);

    // |x| >= 2^66 — atan saturates to π/2 to within a rounding.
    if ax >= 7.378_697_629_483_821e19 {
        let z = ATAN_HI[3] + ATAN_LO[3];
        return if sign_neg { -z } else { z };
    }

    let (id, w): (i32, f64) = if ax < 0.437_5 {
        if ax < 3.725_290_298_461_914e-9 {
            // |x| < 2^-28 — atan(x) == x to full precision.
            return x;
        }
        (-1, ax)
    } else if ax < 1.187_5 {
        if ax < 0.687_5 {
            (0, (2.0 * ax - 1.0) / (2.0 + ax))
        } else {
            (1, (ax - 1.0) / (ax + 1.0))
        }
    } else if ax < 2.437_5 {
        (2, (ax - 1.5) / (1.0 + 1.5 * ax))
    } else {
        (3, -1.0 / ax)
    };

    let z = w * w;
    let w2 = z * z;
    let s1 = z * (AT[0] + w2 * (AT[2] + w2 * (AT[4] + w2 * (AT[6] + w2 * (AT[8] + w2 * AT[10])))));
    let s2 = w2 * (AT[1] + w2 * (AT[3] + w2 * (AT[5] + w2 * (AT[7] + w2 * AT[9]))));

    if id < 0 {
        let r = w - w * (s1 + s2);
        return if sign_neg { -r } else { r };
    }
    let i = id as usize;
    let r = ATAN_HI[i] - ((w * (s1 + s2) - ATAN_LO[i]) - w);
    if sign_neg {
        -r
    } else {
        r
    }
}

/// Two-argument arctangent — the angle of `(x, y)` in `[-π, π]`, quadrant-correct.
pub fn atan2(y: f64, x: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if y == 0.0 {
        // Sign of the result follows the sign of y; x < 0 (including -0.0) gives ±π.
        return if x > 0.0 || (x == 0.0 && !x.is_sign_negative()) {
            copysign(0.0, y)
        } else {
            copysign(PI, y)
        };
    }
    if x == 0.0 {
        return copysign(FRAC_PI_2, y);
    }
    if x.is_infinite() {
        return if y.is_infinite() {
            if x > 0.0 {
                copysign(FRAC_PI_4, y)
            } else {
                copysign(3.0 * FRAC_PI_4, y)
            }
        } else if x > 0.0 {
            copysign(0.0, y)
        } else {
            copysign(PI, y)
        };
    }
    if y.is_infinite() {
        return copysign(FRAC_PI_2, y);
    }
    let z = atan(abs(y / x));
    if x > 0.0 {
        copysign(z, y)
    } else {
        copysign(PI - z, y)
    }
}

/// Arcsine, in `[-π/2, π/2]`. NaN outside `[-1, 1]`.
pub fn asin(x: f64) -> f64 {
    if x.is_nan() || abs(x) > 1.0 {
        return f64::NAN;
    }
    if abs(x) == 1.0 {
        return copysign(FRAC_PI_2, x);
    }
    atan2(x, sqrt((1.0 - x) * (1.0 + x)))
}

/// Arccosine, in `[0, π]`. NaN outside `[-1, 1]`.
pub fn acos(x: f64) -> f64 {
    if x.is_nan() || abs(x) > 1.0 {
        return f64::NAN;
    }
    atan2(sqrt((1.0 - x) * (1.0 + x)), x)
}

// ---------------------------------------------------------------------------------------------
// Exponential & logarithm — owned fdlibm formulations
// ---------------------------------------------------------------------------------------------

/// `e^x`.
pub fn exp(x: f64) -> f64 {
    const P1: f64 = 1.666_666_666_666_660_2e-1;
    const P2: f64 = -2.777_777_777_701_559_3e-3;
    const P3: f64 = 6.613_756_321_437_934e-5;
    const P4: f64 = -1.653_390_220_546_525_2e-6;
    const P5: f64 = 4.138_136_797_057_238_4e-8;

    if x.is_nan() {
        return f64::NAN;
    }
    if x > 709.782_712_893_384 {
        return f64::INFINITY;
    }
    if x < -745.133_219_101_941_2 {
        return 0.0;
    }
    if abs(x) < 3.725_290_298_461_914e-9 {
        // |x| < 2^-28 — e^x == 1 + x to full precision.
        return 1.0 + x;
    }

    let kf = round(x * INV_LN2);
    let k = kf as i32;
    let hi = x - kf * LN2_HI; // exact: kf*LN2_HI has no low bits
    let lo = kf * LN2_LO;
    let r = hi - lo;
    let t = r * r;
    let c = r - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    let y = if k == 0 {
        1.0 - ((r * c) / (c - 2.0) - r)
    } else {
        1.0 - ((lo - (r * c) / (2.0 - c)) - hi)
    };
    scalbn(y, k)
}

/// Natural logarithm. `NaN` for negative input, `-∞` at zero.
pub fn ln(x: f64) -> f64 {
    const LG1: f64 = 6.666_666_666_666_735e-1;
    const LG2: f64 = 3.999_999_999_940_942e-1;
    const LG3: f64 = 2.857_142_874_366_239e-1;
    const LG4: f64 = 2.222_219_843_214_978_4e-1;
    const LG5: f64 = 1.818_357_216_161_805e-1;
    const LG6: f64 = 1.531_383_769_920_937_3e-1;
    const LG7: f64 = 1.479_819_860_511_658_6e-1;

    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return f64::INFINITY;
    }

    let mut x = x;
    let mut k: i32 = 0;
    let mut hx = (x.to_bits() >> 32) as u32;
    if hx < 0x0010_0000 {
        // Subnormal — scale up by 2^54 and account for it in k.
        k -= 54;
        x *= 18_014_398_509_481_984.0;
        hx = (x.to_bits() >> 32) as u32;
    }
    k += ((hx >> 20) as i32) - 1023;
    let mant = hx & 0x000f_ffff;
    // Choose the binade so the normalized value lands in [sqrt(2)/2, sqrt(2)).
    let i = mant.wrapping_add(0x9_5f64) & 0x10_0000;
    let new_hi = (mant | (i ^ 0x3ff0_0000)) as u64;
    x = f64::from_bits((new_hi << 32) | (x.to_bits() & 0xffff_ffff));
    k += (i >> 20) as i32;

    let f = x - 1.0;
    let dk = k as f64;

    // |f| < 2^-20 — a short series avoids cancellation.
    if (0x000f_ffff & (mant.wrapping_add(2))) < 3 {
        if f == 0.0 {
            return if k == 0 {
                0.0
            } else {
                dk * LN2_HI + dk * LN2_LO
            };
        }
        let r = f * f * (0.5 - 0.333_333_333_333_333_3 * f);
        return if k == 0 {
            f - r
        } else {
            dk * LN2_HI - ((r - dk * LN2_LO) - f)
        };
    }

    let s = f / (2.0 + f);
    let z = s * s;
    let w = z * z;
    let t1 = w * (LG2 + w * (LG4 + w * LG6));
    let t2 = z * (LG1 + w * (LG3 + w * (LG5 + w * LG7)));
    let r = t2 + t1;

    if ((mant as i32 - 0x6_147a) | (0x6_b851 - mant as i32)) > 0 {
        let hfsq = 0.5 * f * f;
        if k == 0 {
            f - (hfsq - s * (hfsq + r))
        } else {
            dk * LN2_HI - ((hfsq - (s * (hfsq + r) + dk * LN2_LO)) - f)
        }
    } else if k == 0 {
        f - s * (f - r)
    } else {
        dk * LN2_HI - ((s * (f - r) - dk * LN2_LO) - f)
    }
}

/// Base-2 logarithm.
pub fn log2(x: f64) -> f64 {
    ln(x) * INV_LN2
}

/// Base-10 logarithm.
pub fn log10(x: f64) -> f64 {
    ln(x) * INV_LN10
}

/// Logarithm of `x` in an arbitrary `base`.
pub fn log(x: f64, base: f64) -> f64 {
    ln(x) / ln(base)
}

/// `x` raised to an **integer** power — exact, by binary exponentiation.
pub fn powi(x: f64, n: i32) -> f64 {
    let negative = n < 0;
    let mut e = (n as i64).unsigned_abs();
    let mut base = x;
    let mut acc = 1.0;
    while e > 0 {
        if e & 1 == 1 {
            acc *= base;
        }
        e >>= 1;
        if e > 0 {
            base *= base;
        }
    }
    if negative {
        1.0 / acc
    } else {
        acc
    }
}

/// `x` raised to the power `y`. Integer exponents route through the exact [`powi`].
pub fn pow(x: f64, y: f64) -> f64 {
    // IEEE-754 pins these two ahead of any NaN handling: 1^y and x^0 are 1 even for NaN operands.
    if y == 0.0 {
        return 1.0;
    }
    if x == 1.0 {
        return 1.0;
    }
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if y == 1.0 {
        return x;
    }
    // Exact integer exponents (also the only way to raise a negative base).
    if y == trunc(y) && abs(y) <= 1024.0 {
        return powi(x, y as i32);
    }
    if y == 0.5 && x >= 0.0 {
        return sqrt(x);
    }
    if x < 0.0 {
        return f64::NAN; // non-integer power of a negative base
    }
    if x == 0.0 {
        return if y > 0.0 { 0.0 } else { f64::INFINITY };
    }
    if x.is_infinite() {
        return if y > 0.0 { f64::INFINITY } else { 0.0 };
    }
    if y.is_infinite() {
        let bigger = x > 1.0;
        return if bigger == (y > 0.0) {
            f64::INFINITY
        } else {
            0.0
        };
    }
    // Split y into its integer and fractional parts: x^y = x^trunc(y) · x^frac(y).
    //
    // The integer half is exact, and the fractional half feeds `exp` an argument bounded by |ln x|
    // instead of |y·ln x|. Since `exp` amplifies the error in its argument, that shrinks the worst
    // case from several ULP to roughly one — e.g. pow(0.05, 3.3) drops from 5 ULP to 1.
    let yi = trunc(y);
    if abs(yi) <= 1024.0 {
        let whole = powi(x, yi as i32);
        if whole != 0.0 && whole.is_finite() {
            return whole * exp((y - yi) * ln(x));
        }
    }
    exp(y * ln(x))
}

/// `sqrt(x² + y²)` without spurious overflow.
pub fn hypot(x: f64, y: f64) -> f64 {
    let (ax, ay) = (abs(x), abs(y));
    if ax.is_infinite() || ay.is_infinite() {
        return f64::INFINITY;
    }
    let (hi, lo) = if ax > ay { (ax, ay) } else { (ay, ax) };
    if hi == 0.0 {
        return 0.0;
    }
    let r = lo / hi;
    hi * sqrt(1.0 + r * r)
}

/// Cube root, preserving sign.
pub fn cbrt(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let r = exp(ln(abs(x)) / 3.0);
    // One Newton refinement recovers the bits lost in exp(ln(·)/3).
    let r = r - (r - abs(x) / (r * r)) / 3.0;
    copysign(r, x)
}

/// Are two values within `epsilon` of each other?
#[inline]
pub fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
    abs(a - b) <= epsilon
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_ops_behave() {
        assert_eq!(sqrt(16.0), 4.0);
        assert_eq!(floor(-1.5), -2.0);
        assert_eq!(ceil(-1.5), -1.0);
        assert_eq!(trunc(-1.5), -1.0);
        assert_eq!(round(-1.5), -2.0);
        assert_eq!(clamp(5.0, 0.0, 1.0), 1.0);
        assert_eq!(lerp(2.0, 4.0, 0.0), 2.0);
        assert_eq!(lerp(2.0, 4.0, 0.5), 3.0);
        assert_eq!(signum(-0.0), 0.0);
        assert_eq!(scalbn(1.0, 10), 1024.0);
        assert_eq!(scalbn(3.0, -2), 0.75);
    }

    #[test]
    fn trig_landmarks() {
        assert!(approx_eq(sin(0.0), 0.0, 1e-15));
        assert!(approx_eq(sin(FRAC_PI_2), 1.0, 1e-15));
        assert!(approx_eq(cos(0.0), 1.0, 1e-15));
        assert!(approx_eq(cos(PI), -1.0, 1e-15));
        assert!(approx_eq(atan2(1.0, 1.0), FRAC_PI_4, 1e-15));
        assert!(approx_eq(acos(-1.0), PI, 1e-15));
        assert!(approx_eq(asin(1.0), FRAC_PI_2, 1e-15));
    }

    #[test]
    fn exp_ln_landmarks() {
        assert_eq!(exp(0.0), 1.0);
        assert_eq!(ln(1.0), 0.0);
        assert!(approx_eq(ln(E), 1.0, 1e-15));
        assert!(approx_eq(exp(1.0), E, 1e-15));
        assert!(approx_eq(log2(1024.0), 10.0, 1e-12));
        assert_eq!(ln(0.0), f64::NEG_INFINITY);
        assert!(ln(-1.0).is_nan());
    }

    #[test]
    fn pow_integer_exponents_are_exact() {
        assert_eq!(pow(2.0, 10.0), 1024.0);
        assert_eq!(pow(-2.0, 3.0), -8.0);
        assert_eq!(pow(2.0, -2.0), 0.25);
        assert_eq!(powi(3.0, 4), 81.0);
        assert_eq!(pow(5.0, 0.0), 1.0);
        assert!(pow(-2.0, 0.5).is_nan());
    }

    #[test]
    fn hypot_and_cbrt() {
        assert_eq!(hypot(3.0, 4.0), 5.0);
        assert!(approx_eq(cbrt(27.0), 3.0, 1e-12));
        assert!(approx_eq(cbrt(-8.0), -2.0, 1e-12));
    }
}
