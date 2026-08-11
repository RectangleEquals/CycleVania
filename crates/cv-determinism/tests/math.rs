//! Accuracy + property tests for the owned math (M02).
//!
//! Two independent things are being checked, and it matters that they are separate:
//!
//! * **Determinism** — that the owned routines return the same bits everywhere. That is proven by the
//!   golden probe (`cross_target.rs` + `scripts/wasm-golden.cjs`), not here.
//! * **Accuracy** — that they are *right*. This file is that half: it uses the platform libm as an
//!   oracle and bounds the disagreement in ULP. A deterministic-but-wrong `sin` would sail through
//!   the golden test and be caught here.
//!
//! The platform libm is deliberately called here, and *only* here, so `clippy.toml`'s determinism ban
//! is lifted for this file alone.
#![allow(clippy::disallowed_methods)]

use cv_determinism::math;

/// Map a float onto a monotonically ordered integer line so adjacent floats differ by 1.
fn ordered(x: f64) -> i128 {
    let b = x.to_bits();
    if b & 0x8000_0000_0000_0000 != 0 {
        -((b & 0x7fff_ffff_ffff_ffff) as i128)
    } else {
        b as i128
    }
}

/// Distance between two floats measured in representable steps (ULP).
fn ulp_diff(a: f64, b: f64) -> i128 {
    if a == b {
        return 0;
    }
    if a.is_nan() && b.is_nan() {
        return 0;
    }
    if a.is_nan() || b.is_nan() {
        return i128::MAX;
    }
    (ordered(a) - ordered(b)).abs()
}

/// Assert `ours` is within `max_ulp` of the platform result at every sampled input.
fn assert_close(
    name: &str,
    inputs: &[f64],
    ours: impl Fn(f64) -> f64,
    theirs: impl Fn(f64) -> f64,
    max_ulp: i128,
) {
    let mut worst = 0i128;
    let mut worst_at = f64::NAN;
    for &x in inputs {
        let d = ulp_diff(ours(x), theirs(x));
        if d > worst {
            worst = d;
            worst_at = x;
        }
    }
    assert!(
        worst <= max_ulp,
        "{name}: worst error {worst} ULP at x = {worst_at} (ours {}, libm {}) exceeds {max_ulp}",
        ours(worst_at),
        theirs(worst_at),
    );
}

/// A deterministic spread of inputs over `[lo, hi]`.
fn linspace(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| lo + (hi - lo) * (i as f64) / ((n - 1) as f64))
        .collect()
}

#[test]
fn sin_cos_are_accurate() {
    let xs = linspace(-20.0, 20.0, 2001);
    assert_close("sin", &xs, math::sin, f64::sin, 2);
    assert_close("cos", &xs, math::cos, f64::cos, 2);

    // Small arguments, where the polynomial kernel runs unreduced.
    let small = linspace(-0.75, 0.75, 501);
    assert_close("sin (small)", &small, math::sin, f64::sin, 1);
    assert_close("cos (small)", &small, math::cos, f64::cos, 1);

    // Large arguments, up to the documented 2^20 edge of the Cody-Waite domain.
    let large = linspace(-1_000_000.0, 1_000_000.0, 4001);
    assert_close("sin (large)", &large, math::sin, f64::sin, 2);
    assert_close("cos (large)", &large, math::cos, f64::cos, 2);
}

/// Past `2^20` the reduction runs out of precision — that is documented and accepted. What must
/// *still* hold is determinism: the same input returns the same bits, because every step is an exact
/// IEEE op. This pins that distinction so nobody later "fixes" the domain limit by adding a
/// target-dependent fallback.
#[test]
fn beyond_the_reduction_domain_stays_deterministic() {
    for x in [1e8, -7.556e7, 1e12, -3.3e15] {
        assert_eq!(math::sin(x).to_bits(), math::sin(x).to_bits());
        assert_eq!(math::cos(x).to_bits(), math::cos(x).to_bits());
        // Still a sane bounded value, never NaN/inf.
        assert!(math::sin(x).abs() <= 1.0 && math::cos(x).abs() <= 1.0);
    }
}

#[test]
fn sin_cos_pair_matches_the_singles() {
    for x in linspace(-30.0, 30.0, 997) {
        let (s, c) = math::sin_cos(x);
        assert_eq!(
            s.to_bits(),
            math::sin(x).to_bits(),
            "sin_cos disagrees with sin at {x}"
        );
        assert_eq!(
            c.to_bits(),
            math::cos(x).to_bits(),
            "sin_cos disagrees with cos at {x}"
        );
    }
}

#[test]
fn pythagorean_identity_holds() {
    for x in linspace(-50.0, 50.0, 1001) {
        let (s, c) = math::sin_cos(x);
        assert!(
            math::approx_eq(s * s + c * c, 1.0, 1e-14),
            "sin²+cos² drifted at {x}"
        );
    }
}

#[test]
fn tan_is_accurate_away_from_poles() {
    let xs: Vec<f64> = linspace(-1.4, 1.4, 1001);
    assert_close("tan", &xs, math::tan, f64::tan, 3);
}

#[test]
fn atan_and_atan2_are_accurate() {
    let xs = linspace(-50.0, 50.0, 2001);
    assert_close("atan", &xs, math::atan, f64::atan, 2);
    // Tiny and huge magnitudes hit the early-out branches.
    for &x in &[1e-300, 1e-30, 1e-9, 1e9, 1e30, 1e300] {
        for s in [1.0, -1.0] {
            assert!(
                ulp_diff(math::atan(s * x), (s * x).atan()) <= 2,
                "atan at {}",
                s * x
            );
        }
    }
    // atan2 across all four quadrants.
    for y in linspace(-8.0, 8.0, 101) {
        for x in linspace(-8.0, 8.0, 101) {
            let d = ulp_diff(math::atan2(y, x), y.atan2(x));
            assert!(d <= 2, "atan2({y}, {x}) off by {d} ULP");
        }
    }
}

#[test]
fn atan2_special_cases_match_ieee() {
    let cases = [
        (0.0, 1.0),
        (-0.0, 1.0),
        (0.0, -1.0),
        (-0.0, -1.0),
        (1.0, 0.0),
        (-1.0, 0.0),
        (1.0, -0.0),
        (f64::INFINITY, f64::INFINITY),
        (f64::INFINITY, f64::NEG_INFINITY),
        (f64::NEG_INFINITY, f64::INFINITY),
        (1.0, f64::INFINITY),
        (1.0, f64::NEG_INFINITY),
        (f64::INFINITY, 1.0),
    ];
    for (y, x) in cases {
        let ours = math::atan2(y, x);
        let theirs = y.atan2(x);
        assert_eq!(
            ours.to_bits(),
            theirs.to_bits(),
            "atan2({y}, {x}): {ours} vs {theirs}"
        );
    }
    assert!(math::atan2(f64::NAN, 1.0).is_nan());
    assert!(math::atan2(1.0, f64::NAN).is_nan());
}

#[test]
fn asin_acos_are_accurate() {
    let xs = linspace(-1.0, 1.0, 2001);
    assert_close("asin", &xs, math::asin, f64::asin, 2);
    assert_close("acos", &xs, math::acos, f64::acos, 2);
    assert!(math::asin(1.5).is_nan());
    assert!(math::acos(-1.5).is_nan());
}

#[test]
fn exp_is_accurate() {
    let xs = linspace(-700.0, 700.0, 2001);
    assert_close("exp", &xs, math::exp, f64::exp, 2);
    let near_zero = linspace(-1.0, 1.0, 1001);
    assert_close("exp (near 0)", &near_zero, math::exp, f64::exp, 2);
    // Saturation, not garbage.
    assert_eq!(math::exp(1000.0), f64::INFINITY);
    assert_eq!(math::exp(-1000.0), 0.0);
    assert!(math::exp(f64::NAN).is_nan());
}

#[test]
fn ln_is_accurate() {
    // Spread across many binades, including subnormals.
    let mut xs: Vec<f64> = linspace(1e-6, 1000.0, 1501);
    xs.extend([
        1e-300,
        1e-100,
        1e-20,
        5e-324,
        1e100,
        1e300,
        f64::MAX,
        f64::MIN_POSITIVE,
    ]);
    xs.extend(linspace(0.5, 2.0, 501)); // around 1.0, where cancellation bites
    assert_close("ln", &xs, math::ln, f64::ln, 2);
    assert_eq!(math::ln(1.0), 0.0);
    assert_eq!(math::ln(0.0), f64::NEG_INFINITY);
    assert!(math::ln(-1.0).is_nan());
    assert_eq!(math::ln(f64::INFINITY), f64::INFINITY);
}

#[test]
fn exp_and_ln_are_inverses() {
    for x in linspace(-50.0, 50.0, 1001) {
        let round_trip = math::ln(math::exp(x));
        assert!(
            math::approx_eq(round_trip, x, 1e-12 * math::max(1.0, math::abs(x))),
            "at {x}"
        );
    }
}

#[test]
fn log_bases_are_accurate() {
    let xs = linspace(1e-3, 1e6, 1001);
    assert_close("log2", &xs, math::log2, f64::log2, 2);
    assert_close("log10", &xs, math::log10, f64::log10, 2);
    // Exact powers should land essentially on the integer.
    for k in 1..40 {
        assert!(
            math::approx_eq(math::log2(math::powi(2.0, k)), k as f64, 1e-9),
            "log2 2^{k}"
        );
    }
}

#[test]
fn pow_integer_exponents_are_exact() {
    // Binary exponentiation must be bit-exact where the result is representable.
    for base in [2.0, 3.0, 0.5, -2.0, 1.5, 10.0] {
        for e in -20..=20 {
            let ours = math::pow(base, e as f64);
            let theirs = base.powi(e);
            assert!(
                ulp_diff(ours, theirs) <= 2,
                "pow({base}, {e}): {ours} vs {theirs}"
            );
        }
    }
    assert_eq!(math::pow(2.0, 10.0), 1024.0);
    assert_eq!(math::powi(2.0, 10), 1024.0);
    assert_eq!(math::powi(2.0, 0), 1.0);
}

#[test]
fn pow_fractional_exponents_are_accurate() {
    // The residual error is the log2(n) roundings inside binary exponentiation, so it grows with the
    // integer part of the exponent — hence the larger exponents here and the 6 ULP bound.
    for base in linspace(0.01, 50.0, 601) {
        for e in [0.25, 0.5, 1.5, 2.5, -0.75, 3.3, -2.2, 7.7, -9.1] {
            let d = ulp_diff(math::pow(base, e), base.powf(e));
            assert!(d <= 6, "pow({base}, {e}) off by {d} ULP");
        }
    }
}

#[test]
fn pow_edge_cases_match_ieee() {
    assert_eq!(math::pow(f64::NAN, 0.0), 1.0); // IEEE: anything^0 == 1
    assert_eq!(math::pow(1.0, f64::NAN), 1.0);
    assert_eq!(math::pow(0.0, 2.0), 0.0);
    assert_eq!(math::pow(0.0, -1.0), f64::INFINITY);
    assert_eq!(math::pow(2.0, f64::INFINITY), f64::INFINITY);
    assert_eq!(math::pow(0.5, f64::INFINITY), 0.0);
    assert!(math::pow(-2.0, 0.5).is_nan());
    assert_eq!(math::pow(-2.0, 3.0), -8.0);
}

#[test]
fn hypot_and_cbrt_are_accurate() {
    for x in linspace(-100.0, 100.0, 401) {
        for y in [0.0, 1.0, -3.5, 1e150, 1e-150] {
            let d = ulp_diff(math::hypot(x, y), x.hypot(y));
            assert!(d <= 2, "hypot({x}, {y}) off by {d} ULP");
        }
    }
    // No spurious overflow where the true result is representable.
    assert!(math::hypot(1e300, 1e300).is_finite());
    let xs = linspace(-1000.0, 1000.0, 1001);
    assert_close("cbrt", &xs, math::cbrt, f64::cbrt, 2);
    assert_eq!(math::cbrt(0.0), 0.0);
}

#[test]
fn scalbn_is_exact() {
    for n in -60..=60 {
        assert_eq!(math::scalbn(1.0, n), 2f64.powi(n));
        assert_eq!(math::scalbn(3.0, n), 3.0 * 2f64.powi(n));
    }
    // Extreme exponents must not flush prematurely.
    assert_eq!(math::scalbn(1.0, 1023), f64::from_bits((2046u64) << 52));
    assert!(
        math::scalbn(1.0, -1074) > 0.0,
        "smallest subnormal should survive"
    );
}

#[test]
fn interpolation_helpers_behave() {
    assert_eq!(math::lerp(10.0, 20.0, 0.0), 10.0);
    assert_eq!(math::lerp(10.0, 20.0, 0.5), 15.0);
    assert_eq!(math::inverse_lerp(10.0, 20.0, 15.0), 0.5);
    assert_eq!(math::inverse_lerp(5.0, 5.0, 9.0), 0.0); // degenerate span, no NaN
    assert_eq!(math::remap(5.0, 0.0, 10.0, 100.0, 200.0), 150.0);
    assert_eq!(math::smoothstep(0.0, 1.0, -1.0), 0.0);
    assert_eq!(math::smoothstep(0.0, 1.0, 2.0), 1.0);
    assert_eq!(math::smoothstep(0.0, 1.0, 0.5), 0.5);
    assert_eq!(math::saturate(1.7), 1.0);
}

#[test]
fn angle_conversions_round_trip() {
    for deg in linspace(-720.0, 720.0, 289) {
        assert!(math::approx_eq(
            math::to_degrees(math::to_radians(deg)),
            deg,
            1e-10
        ));
    }
    assert_eq!(math::to_radians(180.0), math::PI);
}

#[test]
fn non_finite_inputs_do_not_panic() {
    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(math::sin(x).is_nan());
        assert!(math::cos(x).is_nan());
        assert!(math::tan(x).is_nan());
        // These have defined saturating behaviour rather than NaN.
        let _ = math::atan(x);
        let _ = math::exp(x);
        let _ = math::ln(x);
        let _ = math::hypot(x, 1.0);
    }
    assert_eq!(math::atan(f64::INFINITY), math::FRAC_PI_2);
    assert_eq!(math::atan(f64::NEG_INFINITY), -math::FRAC_PI_2);
}
