//! The **cross-target determinism probe** — one canonical byte blob exercising the RNG, the owned
//! math, and the geometry kernels.
//!
//! Both sides of the cross-target check compute this same blob: the native test compares it against
//! the committed golden fixture, and `examples/wasm_probe.rs` exports it from a wasm32 module so the
//! Node harness (`scripts/wasm-golden.cjs`) can compare *that* against the very same fixture. Native
//! and WASM agreeing byte-for-byte with one file is the guarantee.
//!
//! Values are appended as **raw IEEE-754 bit patterns**, little-endian — never formatted text, so a
//! single-ULP difference or a NaN payload cannot hide behind rounding in a decimal rendering.
//!
//! This blob is append-only in spirit: adding cases at the end is a deliberate re-bless; reordering or
//! changing existing cases invalidates the fixture and must be reviewed as a determinism change.

use crate::geom::{Aabb, Quat, Transform, Vec3};
use crate::math;
use crate::Rng;

/// Append a float's exact bit pattern.
fn push_f64(out: &mut Vec<u8>, v: f64) {
    out.extend_from_slice(&v.to_bits().to_le_bytes());
}

/// Append a vector's three components.
fn push_vec3(out: &mut Vec<u8>, v: Vec3) {
    push_f64(out, v.x);
    push_f64(out, v.y);
    push_f64(out, v.z);
}

/// The inputs every unary transcendental is sampled at — spread across sign, magnitude, and the
/// argument-reduction boundaries (±π/4, ±π/2, multi-turn angles).
const SAMPLES: [f64; 24] = [
    0.0,
    1.0,
    -1.0,
    0.5,
    -0.5,
    0.25,
    math::FRAC_PI_4, // the kernel boundary
    math::FRAC_PI_2, // a reduction pivot
    math::PI,
    math::TAU,
    -math::PI,
    2.0,
    -2.0,
    10.0,
    100.0,
    1000.0, // deep into argument reduction
    1e-8,
    1e8,
    0.1,
    0.9,
    1.5,
    -7.25,
    123.456,
    -0.333_333_333_333_333_3,
];

/// Compute the canonical determinism blob.
pub fn determinism_probe() -> Vec<u8> {
    let mut out = Vec::new();

    // --- 1. Owned math over the sample set -----------------------------------------------------
    for &x in SAMPLES.iter() {
        push_f64(&mut out, math::sin(x));
        push_f64(&mut out, math::cos(x));
        push_f64(&mut out, math::tan(x));
        push_f64(&mut out, math::atan(x));
        push_f64(&mut out, math::exp(x));
        push_f64(&mut out, math::ln(x));
        push_f64(&mut out, math::sqrt(x));
        push_f64(&mut out, math::cbrt(x));
    }
    // Domain-restricted inverses, plus the two-argument forms.
    for &x in SAMPLES.iter() {
        let t = math::clamp(x, -1.0, 1.0);
        push_f64(&mut out, math::asin(t));
        push_f64(&mut out, math::acos(t));
        push_f64(&mut out, math::atan2(x, 1.5));
        push_f64(&mut out, math::atan2(-2.5, x));
        push_f64(&mut out, math::pow(math::abs(x) + 0.5, 2.5));
        push_f64(&mut out, math::pow(2.0, x));
        push_f64(&mut out, math::hypot(x, 3.0));
        push_f64(&mut out, math::smoothstep(-1.0, 1.0, x));
    }

    // --- 2. The RNG: root stream, labelled forks, and every distribution ------------------------
    let root = Rng::new(0x005E_ED0F_C1CE);
    let mut stream = root.fork("probe");
    for _ in 0..32 {
        out.extend_from_slice(&stream.next_u64().to_le_bytes());
    }
    let mut dist = root.fork("distributions");
    for _ in 0..32 {
        push_f64(&mut out, dist.next_f64());
        push_f64(&mut out, dist.uniform(-10.0, 10.0));
        push_f64(&mut out, dist.jitter(2.5));
        out.extend_from_slice(&dist.below(1000).to_le_bytes());
        out.extend_from_slice(&(dist.weighted_choice(&[1.0, 2.0, 3.0, 4.0]) as u64).to_le_bytes());
    }
    // Fork identity must not depend on traversal order or parent consumption.
    for label in ["a", "b", "enemies", "items", ""] {
        out.extend_from_slice(&root.fork(label).key().to_le_bytes());
    }
    for i in 0..8u64 {
        out.extend_from_slice(&root.fork_index(i).key().to_le_bytes());
    }
    // A shuffle exercises the rejection loop over a non-power-of-two bound.
    let mut shuffled: Vec<u32> = (0..48).collect();
    root.fork("shuffle").shuffle(&mut shuffled);
    for v in &shuffled {
        out.extend_from_slice(&v.to_le_bytes());
    }

    // --- 3. Geometry kernels --------------------------------------------------------------------
    let a = Vec3::new(1.5, -2.25, 3.125);
    let b = Vec3::new(-0.75, 4.5, 0.0625);
    push_vec3(&mut out, a + b);
    push_vec3(&mut out, a.cross(b));
    push_f64(&mut out, a.dot(b));
    push_f64(&mut out, a.length());
    push_vec3(&mut out, a.normalized());
    push_f64(&mut out, a.angle_to(b));
    push_vec3(&mut out, a.reflect(b.normalized()));
    push_vec3(&mut out, a.project_onto(b));
    push_vec3(&mut out, a.lerp(b, 0.375));

    let q = Quat::from_axis_angle(Vec3::new(0.3, -0.6, 0.75), 1.234_567);
    push_f64(&mut out, q.x);
    push_f64(&mut out, q.y);
    push_f64(&mut out, q.z);
    push_f64(&mut out, q.w);
    push_vec3(&mut out, q.rotate(a));
    push_vec3(&mut out, q.inverse().rotate(a));

    let t = Transform::new(b, q, Vec3::new(2.0, 0.5, 1.25));
    push_vec3(&mut out, t.transform_point(a));
    push_vec3(&mut out, t.transform_vector(a));
    push_vec3(&mut out, t.inverse().transform_point(a));
    let composed = t.compose(&Transform::from_translation(a));
    push_vec3(&mut out, composed.transform_point(b));

    let bounds = Aabb::from_center_extents(a, Vec3::new(2.0, 3.0, 1.0));
    push_vec3(&mut out, bounds.min);
    push_vec3(&mut out, bounds.max);
    push_f64(&mut out, bounds.volume());
    push_f64(&mut out, bounds.surface_area());
    push_f64(&mut out, bounds.distance_to_point(b));
    let rotated = bounds.transformed(&t);
    push_vec3(&mut out, rotated.min);
    push_vec3(&mut out, rotated.max);

    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn probe_is_stable_within_a_run() {
        assert_eq!(super::determinism_probe(), super::determinism_probe());
    }

    #[test]
    fn probe_is_not_trivially_empty() {
        assert!(super::determinism_probe().len() > 2000);
    }
}
