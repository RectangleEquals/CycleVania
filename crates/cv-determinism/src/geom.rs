//! Deterministic geometry kernels — the numeric core behind the `Vec3` / `Transform` / `AABB` value
//! types CVScript exposes as immutable `api class`es at M19.
//!
//! Every routine here is built from [`crate::math`], so the whole set inherits the float contract:
//! bit-identical on native and wasm32. These are plain `Copy` value types with no identity — they are
//! *not* `Object`s and never live in the arena.
//!
//! Arithmetic is exposed through the standard operator traits (`a + b`, `v * 2.0`); `Vec3 * Vec3` is
//! component-wise. Composition of rotations is `q1 * q2`, but [`Transform::compose`] stays a named
//! method — transform ordering is confusing enough that an explicit name beats an operator.
//!
//! World scale is 1 unit = 1 m by default (host-modifiable via `World.scale`); nothing here assumes it.

use crate::math;
use std::ops::{Add, Div, Mul, Neg, Sub};

// ---------------------------------------------------------------------------------------------
// Vec3
// ---------------------------------------------------------------------------------------------

/// A 3-component vector — a position, direction, extent, or scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    /// All components zero.
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    /// All components one.
    pub const ONE: Vec3 = Vec3 {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
    /// The +X axis.
    pub const X: Vec3 = Vec3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    /// The +Y axis.
    pub const Y: Vec3 = Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    /// The +Z axis (up, by convention).
    pub const Z: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    /// Construct from components.
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    /// The same value in every component.
    pub const fn splat(v: f64) -> Self {
        Vec3 { x: v, y: v, z: v }
    }

    /// Multiply every component by a scalar. (Same as `self * s`; named for readability at call sites.)
    pub fn scale(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }

    /// Dot product.
    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// Cross product (right-handed).
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    /// Squared length — prefer this when comparing magnitudes (no `sqrt`).
    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    /// Euclidean length.
    pub fn length(self) -> f64 {
        math::sqrt(self.length_squared())
    }

    /// Distance to another point.
    pub fn distance(self, o: Vec3) -> f64 {
        (self - o).length()
    }

    /// Squared distance to another point.
    pub fn distance_squared(self, o: Vec3) -> f64 {
        (self - o).length_squared()
    }

    /// Unit vector in the same direction. Returns [`Vec3::ZERO`] for a zero-length input.
    pub fn normalized(self) -> Vec3 {
        let len = self.length();
        if len == 0.0 {
            Vec3::ZERO
        } else {
            self.scale(1.0 / len)
        }
    }

    /// Is this vector unit length, within `epsilon`?
    pub fn is_normalized(self, epsilon: f64) -> bool {
        math::abs(self.length_squared() - 1.0) <= epsilon
    }

    /// Component-wise linear interpolation.
    pub fn lerp(self, o: Vec3, t: f64) -> Vec3 {
        Vec3::new(
            math::lerp(self.x, o.x, t),
            math::lerp(self.y, o.y, t),
            math::lerp(self.z, o.z, t),
        )
    }

    /// Component-wise minimum.
    pub fn min(self, o: Vec3) -> Vec3 {
        Vec3::new(
            math::min(self.x, o.x),
            math::min(self.y, o.y),
            math::min(self.z, o.z),
        )
    }

    /// Component-wise maximum.
    pub fn max(self, o: Vec3) -> Vec3 {
        Vec3::new(
            math::max(self.x, o.x),
            math::max(self.y, o.y),
            math::max(self.z, o.z),
        )
    }

    /// Component-wise absolute value.
    pub fn abs(self) -> Vec3 {
        Vec3::new(math::abs(self.x), math::abs(self.y), math::abs(self.z))
    }

    /// The largest component.
    pub fn max_component(self) -> f64 {
        math::max(self.x, math::max(self.y, self.z))
    }

    /// The smallest component.
    pub fn min_component(self) -> f64 {
        math::min(self.x, math::min(self.y, self.z))
    }

    /// Reflect this vector about a surface `normal` (which should be unit length).
    ///
    /// The workhorse behind `ctx.reflect` — bouncing a laser, a projectile, or a jump arc off a
    /// surface (M11).
    pub fn reflect(self, normal: Vec3) -> Vec3 {
        self - normal.scale(2.0 * self.dot(normal))
    }

    /// Project this vector onto `onto`. Returns [`Vec3::ZERO`] if `onto` is degenerate.
    pub fn project_onto(self, onto: Vec3) -> Vec3 {
        let d = onto.length_squared();
        if d == 0.0 {
            Vec3::ZERO
        } else {
            onto.scale(self.dot(onto) / d)
        }
    }

    /// The component of this vector perpendicular to `normal` — i.e. sliding along a surface.
    pub fn reject_from(self, normal: Vec3) -> Vec3 {
        self - self.project_onto(normal)
    }

    /// The unsigned angle to another vector, in radians `[0, π]`.
    pub fn angle_to(self, o: Vec3) -> f64 {
        let denom = math::sqrt(self.length_squared() * o.length_squared());
        if denom == 0.0 {
            return 0.0;
        }
        math::acos(math::clamp(self.dot(o) / denom, -1.0, 1.0))
    }

    /// Are all components finite?
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// Are two vectors within `epsilon` per component?
    pub fn approx_eq(self, o: Vec3, epsilon: f64) -> bool {
        math::approx_eq(self.x, o.x, epsilon)
            && math::approx_eq(self.y, o.y, epsilon)
            && math::approx_eq(self.z, o.z, epsilon)
    }

    /// As a `[x, y, z]` array.
    pub fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

/// Component-wise multiplication (a non-uniform scale).
impl Mul for Vec3 {
    type Output = Vec3;
    fn mul(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f64) -> Vec3 {
        self.scale(s)
    }
}

/// Component-wise division.
impl Div for Vec3 {
    type Output = Vec3;
    fn div(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x / o.x, self.y / o.y, self.z / o.z)
    }
}

impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f64) -> Vec3 {
        Vec3::new(self.x / s, self.y / s, self.z / s)
    }
}

// ---------------------------------------------------------------------------------------------
// Quat
// ---------------------------------------------------------------------------------------------

/// A unit quaternion representing a rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Quat {
    /// The identity rotation.
    pub const IDENTITY: Quat = Quat {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// Construct from raw components (not normalized).
    pub const fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Quat { x, y, z, w }
    }

    /// A rotation of `angle` radians about `axis` (which is normalized internally).
    pub fn from_axis_angle(axis: Vec3, angle: f64) -> Quat {
        let a = axis.normalized();
        if a == Vec3::ZERO {
            return Quat::IDENTITY;
        }
        let half = angle * 0.5;
        let (s, c) = math::sin_cos(half);
        Quat::new(a.x * s, a.y * s, a.z * s, c)
    }

    /// Squared magnitude.
    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    /// Magnitude.
    pub fn length(self) -> f64 {
        math::sqrt(self.length_squared())
    }

    /// Unit quaternion; identity if degenerate.
    pub fn normalized(self) -> Quat {
        let len = self.length();
        if len == 0.0 {
            Quat::IDENTITY
        } else {
            let inv = 1.0 / len;
            Quat::new(self.x * inv, self.y * inv, self.z * inv, self.w * inv)
        }
    }

    /// The conjugate — the inverse rotation for a unit quaternion.
    pub fn conjugate(self) -> Quat {
        Quat::new(-self.x, -self.y, -self.z, self.w)
    }

    /// The inverse rotation.
    pub fn inverse(self) -> Quat {
        let d = self.length_squared();
        if d == 0.0 {
            return Quat::IDENTITY;
        }
        let c = self.conjugate();
        let inv = 1.0 / d;
        Quat::new(c.x * inv, c.y * inv, c.z * inv, c.w * inv)
    }

    /// Rotate a vector by this quaternion.
    pub fn rotate(self, v: Vec3) -> Vec3 {
        // v + 2w(q × v) + 2(q × (q × v)) — avoids building a matrix.
        let q = Vec3::new(self.x, self.y, self.z);
        let t = q.cross(v).scale(2.0);
        v + t.scale(self.w) + q.cross(t)
    }

    /// Are two rotations within `epsilon` per component?
    pub fn approx_eq(self, o: Quat, epsilon: f64) -> bool {
        math::approx_eq(self.x, o.x, epsilon)
            && math::approx_eq(self.y, o.y, epsilon)
            && math::approx_eq(self.z, o.z, epsilon)
            && math::approx_eq(self.w, o.w, epsilon)
    }
}

/// Compose rotations — `self` is applied *after* `o`.
impl Mul for Quat {
    type Output = Quat;
    fn mul(self, o: Quat) -> Quat {
        Quat::new(
            self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
            self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
        )
    }
}

// ---------------------------------------------------------------------------------------------
// Transform
// ---------------------------------------------------------------------------------------------

/// A TRS transform — translation, rotation, then non-uniform scale.
///
/// This is the placement record the pipeline emits for every mesh/actor (M06 mesh metadata).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    /// No translation, no rotation, unit scale.
    pub const IDENTITY: Transform = Transform {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// Construct from all three parts.
    pub const fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Transform {
            translation,
            rotation,
            scale,
        }
    }

    /// A pure translation.
    pub const fn from_translation(t: Vec3) -> Self {
        Transform {
            translation: t,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    /// A pure rotation.
    pub const fn from_rotation(r: Quat) -> Self {
        Transform {
            translation: Vec3::ZERO,
            rotation: r,
            scale: Vec3::ONE,
        }
    }

    /// A pure scale.
    pub const fn from_scale(s: Vec3) -> Self {
        Transform {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: s,
        }
    }

    /// Apply to a **point** — scale, then rotate, then translate.
    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        self.rotation.rotate(p * self.scale) + self.translation
    }

    /// Apply to a **direction** — scale and rotate, but do not translate.
    pub fn transform_vector(&self, v: Vec3) -> Vec3 {
        self.rotation.rotate(v * self.scale)
    }

    /// Compose: the result applies `other` first, then `self`.
    ///
    /// Deliberately a named method rather than `Mul` — transform ordering is a classic source of
    /// bugs, and `a.compose(&b)` states the order that `a * b` leaves to memory.
    pub fn compose(&self, other: &Transform) -> Transform {
        Transform {
            translation: self.transform_point(other.translation),
            rotation: self.rotation * other.rotation,
            scale: self.scale * other.scale,
        }
    }

    /// The inverse transform. Requires a non-zero scale on every axis.
    pub fn inverse(&self) -> Transform {
        let inv_scale = Vec3::new(
            if self.scale.x == 0.0 {
                0.0
            } else {
                1.0 / self.scale.x
            },
            if self.scale.y == 0.0 {
                0.0
            } else {
                1.0 / self.scale.y
            },
            if self.scale.z == 0.0 {
                0.0
            } else {
                1.0 / self.scale.z
            },
        );
        let inv_rot = self.rotation.inverse();
        Transform {
            translation: inv_rot.rotate(-self.translation) * inv_scale,
            rotation: inv_rot,
            scale: inv_scale,
        }
    }

    /// Are two transforms within `epsilon` componentwise?
    pub fn approx_eq(&self, o: &Transform, epsilon: f64) -> bool {
        self.translation.approx_eq(o.translation, epsilon)
            && self.rotation.approx_eq(o.rotation, epsilon)
            && self.scale.approx_eq(o.scale, epsilon)
    }
}

// ---------------------------------------------------------------------------------------------
// Aabb
// ---------------------------------------------------------------------------------------------

/// An axis-aligned bounding box — the coarse spatial envelope the pipeline reasons about long before
/// any real geometry exists (L4, M14).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// From explicit corners; components are sorted so the result is always well-formed.
    pub fn new(a: Vec3, b: Vec3) -> Self {
        Aabb {
            min: a.min(b),
            max: a.max(b),
        }
    }

    /// From a center and **half**-extents.
    pub fn from_center_extents(center: Vec3, half_extents: Vec3) -> Self {
        let h = half_extents.abs();
        Aabb {
            min: center - h,
            max: center + h,
        }
    }

    /// The inverted-infinite box — the identity for [`Aabb::union`], so folding points into it works.
    pub fn empty() -> Self {
        Aabb {
            min: Vec3::splat(f64::INFINITY),
            max: Vec3::splat(f64::NEG_INFINITY),
        }
    }

    /// Has this box no volume (e.g. it is still [`Aabb::empty`])?
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }

    /// The midpoint.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max).scale(0.5)
    }

    /// Full width on each axis.
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// Half-width on each axis.
    pub fn half_extents(&self) -> Vec3 {
        self.size().scale(0.5)
    }

    /// Enclosed volume.
    pub fn volume(&self) -> f64 {
        if self.is_empty() {
            return 0.0;
        }
        let s = self.size();
        s.x * s.y * s.z
    }

    /// Total surface area.
    pub fn surface_area(&self) -> f64 {
        if self.is_empty() {
            return 0.0;
        }
        let s = self.size();
        2.0 * (s.x * s.y + s.y * s.z + s.z * s.x)
    }

    /// Is `p` inside (inclusive of the faces)?
    pub fn contains_point(&self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    /// Does this box fully contain `other`?
    pub fn contains(&self, other: &Aabb) -> bool {
        self.contains_point(other.min) && self.contains_point(other.max)
    }

    /// Do the two boxes overlap (touching faces count)?
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    /// The overlapping region, or `None` when they are disjoint.
    pub fn intersection(&self, other: &Aabb) -> Option<Aabb> {
        if !self.intersects(other) {
            return None;
        }
        Some(Aabb {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        })
    }

    /// The smallest box containing both.
    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// The smallest box containing this one and `p`.
    pub fn extended_to(&self, p: Vec3) -> Aabb {
        Aabb {
            min: self.min.min(p),
            max: self.max.max(p),
        }
    }

    /// Grow (or, negatively, shrink) by `amount` on every axis.
    pub fn expanded(&self, amount: f64) -> Aabb {
        let d = Vec3::splat(amount);
        Aabb {
            min: self.min - d,
            max: self.max + d,
        }
    }

    /// The closest point inside the box to `p`.
    pub fn closest_point(&self, p: Vec3) -> Vec3 {
        Vec3::new(
            math::clamp(p.x, self.min.x, self.max.x),
            math::clamp(p.y, self.min.y, self.max.y),
            math::clamp(p.z, self.min.z, self.max.z),
        )
    }

    /// Distance from `p` to the box; zero when `p` is inside.
    pub fn distance_to_point(&self, p: Vec3) -> f64 {
        self.closest_point(p).distance(p)
    }

    /// The eight corners, in a fixed (deterministic) order.
    pub fn corners(&self) -> [Vec3; 8] {
        let (a, b) = (self.min, self.max);
        [
            Vec3::new(a.x, a.y, a.z),
            Vec3::new(b.x, a.y, a.z),
            Vec3::new(a.x, b.y, a.z),
            Vec3::new(b.x, b.y, a.z),
            Vec3::new(a.x, a.y, b.z),
            Vec3::new(b.x, a.y, b.z),
            Vec3::new(a.x, b.y, b.z),
            Vec3::new(b.x, b.y, b.z),
        ]
    }

    /// The smallest axis-aligned box enclosing this one after `t` is applied.
    pub fn transformed(&self, t: &Transform) -> Aabb {
        if self.is_empty() {
            return *self;
        }
        let mut out = Aabb::empty();
        for c in self.corners() {
            out = out.extended_to(t.transform_point(c));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-12;

    #[test]
    fn vec3_algebra() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vec3::new(5.0, 7.0, 9.0));
        assert_eq!(b - a, Vec3::new(3.0, 3.0, 3.0));
        assert_eq!(-a, Vec3::new(-1.0, -2.0, -3.0));
        assert_eq!(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(a * b, Vec3::new(4.0, 10.0, 18.0)); // component-wise
        assert_eq!(a / 2.0, Vec3::new(0.5, 1.0, 1.5));
        assert_eq!(a.dot(b), 32.0);
        assert_eq!(Vec3::X.cross(Vec3::Y), Vec3::Z);
        assert_eq!(Vec3::new(3.0, 4.0, 0.0).length(), 5.0);
        assert!(a.normalized().is_normalized(EPS));
        assert_eq!(Vec3::ZERO.normalized(), Vec3::ZERO);
    }

    #[test]
    fn reflect_is_mirror_symmetric() {
        // Straight down onto a floor comes straight back up.
        let d = Vec3::new(0.0, 0.0, -1.0);
        assert!(d.reflect(Vec3::Z).approx_eq(Vec3::new(0.0, 0.0, 1.0), EPS));
        // Reflecting twice returns the original.
        let v = Vec3::new(0.3, -0.7, 0.5);
        let n = Vec3::new(1.0, 2.0, 3.0).normalized();
        assert!(v.reflect(n).reflect(n).approx_eq(v, 1e-12));
    }

    #[test]
    fn angle_and_projection() {
        assert!(math::approx_eq(
            Vec3::X.angle_to(Vec3::Y),
            math::FRAC_PI_2,
            1e-12
        ));
        assert!(math::approx_eq(Vec3::X.angle_to(Vec3::X), 0.0, 1e-12));
        let v = Vec3::new(2.0, 3.0, 0.0);
        assert!(v
            .project_onto(Vec3::X)
            .approx_eq(Vec3::new(2.0, 0.0, 0.0), EPS));
        assert!(v
            .reject_from(Vec3::X)
            .approx_eq(Vec3::new(0.0, 3.0, 0.0), EPS));
    }

    #[test]
    fn quat_rotation_preserves_length() {
        let q = Quat::from_axis_angle(Vec3::new(1.0, 1.0, 0.0), 1.234);
        let v = Vec3::new(0.5, -2.0, 3.5);
        assert!(math::approx_eq(q.rotate(v).length(), v.length(), 1e-12));
        // Quarter turn about Z takes +X to +Y.
        let qz = Quat::from_axis_angle(Vec3::Z, math::FRAC_PI_2);
        assert!(qz.rotate(Vec3::X).approx_eq(Vec3::Y, 1e-12));
        // Rotating then un-rotating is the identity.
        assert!(q.inverse().rotate(q.rotate(v)).approx_eq(v, 1e-12));
    }

    #[test]
    fn quat_composition_matches_sequential_rotation() {
        let q1 = Quat::from_axis_angle(Vec3::Z, math::FRAC_PI_2);
        let q2 = Quat::from_axis_angle(Vec3::X, math::FRAC_PI_2);
        let v = Vec3::new(1.0, 2.0, 3.0);
        // (q1 * q2) applies q2 first.
        assert!((q1 * q2)
            .rotate(v)
            .approx_eq(q1.rotate(q2.rotate(v)), 1e-12));
    }

    #[test]
    fn transform_round_trips() {
        let t = Transform::new(
            Vec3::new(3.0, -1.0, 2.0),
            Quat::from_axis_angle(Vec3::new(0.0, 1.0, 1.0), 0.77),
            Vec3::new(2.0, 2.0, 2.0),
        );
        let p = Vec3::new(1.5, 4.0, -2.5);
        assert!(t
            .inverse()
            .transform_point(t.transform_point(p))
            .approx_eq(p, 1e-10));
        // Composition matches applying them in sequence.
        let u = Transform::from_translation(Vec3::new(1.0, 1.0, 1.0));
        assert!(t
            .compose(&u)
            .transform_point(p)
            .approx_eq(t.transform_point(u.transform_point(p)), 1e-10));
    }

    #[test]
    fn aabb_basics() {
        let b = Aabb::new(Vec3::ZERO, Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(b.center(), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(b.volume(), 48.0);
        assert!(b.contains_point(Vec3::new(1.0, 1.0, 1.0)));
        assert!(!b.contains_point(Vec3::new(-1.0, 1.0, 1.0)));
        assert_eq!(
            b.closest_point(Vec3::new(-5.0, 2.0, 3.0)),
            Vec3::new(0.0, 2.0, 3.0)
        );
        assert_eq!(b.distance_to_point(Vec3::new(-3.0, 2.0, 3.0)), 3.0);
        // Corners are ordered deterministically and all lie inside.
        assert!(b.corners().iter().all(|c| b.contains_point(*c)));
    }

    #[test]
    fn aabb_set_ops() {
        let a = Aabb::new(Vec3::ZERO, Vec3::splat(2.0));
        let b = Aabb::new(Vec3::splat(1.0), Vec3::splat(3.0));
        assert!(a.intersects(&b));
        assert_eq!(
            a.intersection(&b).unwrap(),
            Aabb::new(Vec3::splat(1.0), Vec3::splat(2.0))
        );
        assert_eq!(a.union(&b), Aabb::new(Vec3::ZERO, Vec3::splat(3.0)));
        let far = Aabb::new(Vec3::splat(10.0), Vec3::splat(11.0));
        assert!(!a.intersects(&far));
        assert!(a.intersection(&far).is_none());
        // empty() is the union identity.
        assert_eq!(Aabb::empty().union(&a), a);
        assert!(Aabb::empty().is_empty());
    }

    #[test]
    fn aabb_transformed_encloses_all_corners() {
        let b = Aabb::new(Vec3::ZERO, Vec3::splat(2.0));
        let t = Transform::new(
            Vec3::new(5.0, 0.0, 0.0),
            Quat::from_axis_angle(Vec3::Z, math::FRAC_PI_4),
            Vec3::ONE,
        );
        let out = b.transformed(&t);
        for c in b.corners() {
            assert!(out.expanded(1e-9).contains_point(t.transform_point(c)));
        }
    }
}
