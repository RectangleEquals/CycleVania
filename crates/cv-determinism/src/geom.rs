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
//! # `Transform` vs. `Mat4` — which to reach for
//!
//! [`Transform`] (TRS) is the **canonical placement record**: decomposable, serializable into the
//! descriptor as position/rotation/scale fields, legible in the editor inspector, and interpolatable.
//! Use it for everything the pipeline places.
//!
//! [`Mat4`] is the **general affine escape hatch**, needed for exactly two things TRS provably cannot
//! express:
//!
//! 1. **Composition under non-uniform scale.** Rotating something inside a non-uniformly scaled parent
//!    produces shear. `Transform::compose` documents (and `debug_assert`s) when it is exact; `Mat4`
//!    multiplication always is.
//! 2. **Mirroring.** A reflection has determinant −1; a unit quaternion is always +1. Producing the
//!    handed variant of a room or kit piece needs [`Mat4::from_reflection`].
//!
//! Convert freely with `Mat4::from(transform)` and [`Mat4::to_transform`] — the latter returns `None`
//! rather than silently approximating when the matrix contains shear.
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

    /// Is the scale the same on all three axes?
    pub fn has_uniform_scale(&self) -> bool {
        self.scale.x == self.scale.y && self.scale.y == self.scale.z
    }

    /// Compose: the result applies `other` first, then `self`.
    ///
    /// Deliberately a named method rather than `Mul` — transform ordering is a classic source of
    /// bugs, and `a.compose(&b)` states the order that `a * b` leaves to memory.
    ///
    /// # Exactness
    ///
    /// **TRS is not closed under composition.** This is exact when *either*:
    ///
    /// * `self` (the outer transform) has **uniform scale** — a scalar commutes with rotation, so
    ///   the parts separate cleanly; or
    /// * `other` (the inner transform) has **no rotation** — nothing mixes the scaled axes.
    ///
    /// Outside those cases the true composition contains **shear**, which no TRS can represent, and
    /// the result is an approximation. Use [`Mat4`] for the general case:
    ///
    /// ```
    /// # use cv_determinism::geom::{Mat4, Quat, Transform, Vec3};
    /// # let (outer, inner) = (Transform::IDENTITY, Transform::IDENTITY);
    /// let exact: Mat4 = Mat4::from(outer) * Mat4::from(inner);
    /// ```
    ///
    /// A `debug_assert` flags the lossy case during development; release builds take the fast path
    /// silently, so callers that *intend* an approximation should say so in a comment.
    pub fn compose(&self, other: &Transform) -> Transform {
        debug_assert!(
            self.has_uniform_scale() || other.rotation == Quat::IDENTITY,
            "Transform::compose is lossy here: composing a rotated inner transform inside a \
             non-uniformly scaled outer one produces shear, which TRS cannot represent. \
             Use Mat4::from(outer) * Mat4::from(inner) instead."
        );
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
// Mat4
// ---------------------------------------------------------------------------------------------

/// A 4×4 affine matrix — the **general** transform, where [`Transform`] is the *decomposed* one.
///
/// Storage is **column-major** (`m[col * 4 + row]`), matching glTF, OpenGL and Three.js, so a matrix
/// can be handed to a host or written into mesh metadata without a layout conversion.
///
/// # When you need this instead of `Transform`
///
/// `Transform` (translate-rotate-scale) is the canonical placement record: decomposable, legible in
/// the descriptor and editor, and interpolatable. But TRS is **not closed under composition** — it
/// cannot express two things the generator legitimately produces:
///
/// * **Nested non-uniform scale.** Composing a rotated child inside a non-uniformly scaled parent
///   produces *shear*, which no TRS can represent. (Unity has the same limitation and simply skews
///   visually.) [`Transform::compose`] is exact only in the cases it documents; `Mat4` multiplication
///   is exact always.
/// * **Mirroring.** A reflection has determinant −1, and a unit quaternion can only ever represent a
///   rotation (determinant +1). Mirroring a room, corridor, or kit piece to get its handed variant is
///   an ordinary level-generation move, and [`Mat4::from_reflection`] is the only way to express it.
///
/// Every operation here is built from exact IEEE ops, so `Mat4` carries the same determinism contract
/// as the rest of the module.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    /// Column-major elements: `m[col * 4 + row]`.
    pub m: [f64; 16],
}

impl Mat4 {
    /// The identity matrix.
    pub const IDENTITY: Mat4 = Mat4 {
        m: [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ],
    };

    /// All elements zero.
    pub const ZERO: Mat4 = Mat4 { m: [0.0; 16] };

    /// From a column-major array (glTF / OpenGL / Three.js layout).
    pub const fn from_cols_array(m: [f64; 16]) -> Self {
        Mat4 { m }
    }

    /// As a column-major array.
    pub fn to_cols_array(&self) -> [f64; 16] {
        self.m
    }

    /// Element at `(row, col)`.
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.m[col * 4 + row]
    }

    /// Set the element at `(row, col)`.
    #[inline]
    pub fn set(&mut self, row: usize, col: usize, v: f64) {
        self.m[col * 4 + row] = v;
    }

    /// A pure translation.
    pub fn from_translation(t: Vec3) -> Mat4 {
        let mut r = Mat4::IDENTITY;
        r.m[12] = t.x;
        r.m[13] = t.y;
        r.m[14] = t.z;
        r
    }

    /// A pure (non-uniform) scale.
    pub fn from_scale(s: Vec3) -> Mat4 {
        let mut r = Mat4::IDENTITY;
        r.m[0] = s.x;
        r.m[5] = s.y;
        r.m[10] = s.z;
        r
    }

    /// A pure rotation.
    pub fn from_rotation(q: Quat) -> Mat4 {
        let (x, y, z, w) = (q.x, q.y, q.z, q.w);
        let (x2, y2, z2) = (x + x, y + y, z + z);
        let (xx, xy, xz) = (x * x2, x * y2, x * z2);
        let (yy, yz, zz) = (y * y2, y * z2, z * z2);
        let (wx, wy, wz) = (w * x2, w * y2, w * z2);
        Mat4 {
            m: [
                1.0 - (yy + zz),
                xy + wz,
                xz - wy,
                0.0,
                xy - wz,
                1.0 - (xx + zz),
                yz + wx,
                0.0,
                xz + wy,
                yz - wx,
                1.0 - (xx + yy),
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            ],
        }
    }

    /// The matrix form of a TRS [`Transform`] — scale, then rotate, then translate.
    pub fn from_transform(t: &Transform) -> Mat4 {
        let mut r = Mat4::from_rotation(t.rotation);
        // Scale each basis column (pre-multiplying by the scale).
        for (col, s) in [t.scale.x, t.scale.y, t.scale.z].into_iter().enumerate() {
            r.m[col * 4] *= s;
            r.m[col * 4 + 1] *= s;
            r.m[col * 4 + 2] *= s;
        }
        r.m[12] = t.translation.x;
        r.m[13] = t.translation.y;
        r.m[14] = t.translation.z;
        r
    }

    /// **Mirror across the plane through the origin with unit `normal`.**
    ///
    /// This is the operation a quaternion cannot express: the result has determinant −1. Useful for
    /// generating the handed variant of a room, corridor, or kit piece. Note that mirroring flips
    /// triangle winding, so a host consuming the mesh metadata may need to flip face orientation —
    /// query [`Mat4::is_mirroring`] to know.
    pub fn from_reflection(normal: Vec3) -> Mat4 {
        let n = normal.normalized();
        if n == Vec3::ZERO {
            return Mat4::IDENTITY;
        }
        // I - 2·n·nᵀ (Householder), extended affinely.
        let (x, y, z) = (n.x, n.y, n.z);
        Mat4 {
            m: [
                1.0 - 2.0 * x * x,
                -2.0 * x * y,
                -2.0 * x * z,
                0.0,
                -2.0 * y * x,
                1.0 - 2.0 * y * y,
                -2.0 * y * z,
                0.0,
                -2.0 * z * x,
                -2.0 * z * y,
                1.0 - 2.0 * z * z,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            ],
        }
    }

    /// Mirror across an arbitrary plane — one with unit `normal` passing through `point`.
    pub fn from_reflection_plane(normal: Vec3, point: Vec3) -> Mat4 {
        Mat4::from_translation(point)
            * Mat4::from_reflection(normal)
            * Mat4::from_translation(-point)
    }

    /// Apply to a **point** (implicit `w = 1`, so translation applies).
    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let m = &self.m;
        Vec3::new(
            m[0] * p.x + m[4] * p.y + m[8] * p.z + m[12],
            m[1] * p.x + m[5] * p.y + m[9] * p.z + m[13],
            m[2] * p.x + m[6] * p.y + m[10] * p.z + m[14],
        )
    }

    /// Apply to a **direction** (implicit `w = 0`, so translation is ignored).
    pub fn transform_vector(&self, v: Vec3) -> Vec3 {
        let m = &self.m;
        Vec3::new(
            m[0] * v.x + m[4] * v.y + m[8] * v.z,
            m[1] * v.x + m[5] * v.y + m[9] * v.z,
            m[2] * v.x + m[6] * v.y + m[10] * v.z,
        )
    }

    /// The transpose.
    pub fn transpose(&self) -> Mat4 {
        let mut r = Mat4::ZERO;
        for row in 0..4 {
            for col in 0..4 {
                r.set(col, row, self.get(row, col));
            }
        }
        r
    }

    /// The determinant. Negative means the transform **flips handedness** (see [`Mat4::is_mirroring`]).
    pub fn determinant(&self) -> f64 {
        let c = self.cofactor_column();
        let m = &self.m;
        m[0] * c[0] + m[1] * c[1] + m[2] * c[2] + m[3] * c[3]
    }

    /// Does this transform flip handedness (determinant < 0)? Mirrored geometry needs its triangle
    /// winding reversed by the consuming host.
    pub fn is_mirroring(&self) -> bool {
        self.determinant() < 0.0
    }

    /// The inverse, or `None` when the matrix is singular.
    pub fn inverse(&self) -> Option<Mat4> {
        let inv = self.adjugate();
        let m = &self.m;
        let det = m[0] * inv.m[0] + m[1] * inv.m[4] + m[2] * inv.m[8] + m[3] * inv.m[12];
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let inv_det = 1.0 / det;
        let mut out = inv;
        for v in out.m.iter_mut() {
            *v *= inv_det;
        }
        Some(out)
    }

    /// Decompose back into a TRS [`Transform`].
    ///
    /// Returns `None` when the matrix contains **shear** (the basis columns are not mutually
    /// orthogonal) or a non-affine bottom row — neither is representable as TRS. A **mirror** *is*
    /// representable, via a negative scale; by convention the sign is placed on X.
    ///
    /// The failure is deliberately visible rather than silently approximated: a lossy decomposition
    /// is exactly the kind of hidden behaviour that makes generated placements untraceable.
    pub fn to_transform(&self) -> Option<Transform> {
        let m = &self.m;
        // Must be affine: bottom row exactly [0, 0, 0, 1].
        if m[3] != 0.0 || m[7] != 0.0 || m[11] != 0.0 || m[15] != 1.0 {
            return None;
        }
        let mut cx = Vec3::new(m[0], m[1], m[2]);
        let cy = Vec3::new(m[4], m[5], m[6]);
        let cz = Vec3::new(m[8], m[9], m[10]);

        let (lx, ly, lz) = (cx.length(), cy.length(), cz.length());
        if lx == 0.0 || ly == 0.0 || lz == 0.0 {
            return None; // degenerate: rotation is unrecoverable
        }
        // Reject shear — the normalized basis must be orthogonal.
        const ORTHO_TOL: f64 = 1e-9;
        let (nx, ny, nz) = (cx.scale(1.0 / lx), cy.scale(1.0 / ly), cz.scale(1.0 / lz));
        if math::abs(nx.dot(ny)) > ORTHO_TOL
            || math::abs(ny.dot(nz)) > ORTHO_TOL
            || math::abs(nz.dot(nx)) > ORTHO_TOL
        {
            return None;
        }

        // A mirror shows up as a left-handed basis; fold the sign into the X scale.
        let mut scale = Vec3::new(lx, ly, lz);
        if nx.cross(ny).dot(nz) < 0.0 {
            scale.x = -scale.x;
            cx = -cx;
        }
        let rot_cols = [cx.scale(1.0 / scale.x.abs()), ny, nz];
        let rotation = quat_from_basis(rot_cols[0], rot_cols[1], rot_cols[2]);

        Some(Transform {
            translation: Vec3::new(m[12], m[13], m[14]),
            rotation,
            scale,
        })
    }

    /// Are two matrices within `epsilon` element-wise?
    pub fn approx_eq(&self, o: &Mat4, epsilon: f64) -> bool {
        self.m
            .iter()
            .zip(o.m.iter())
            .all(|(a, b)| math::approx_eq(*a, *b, epsilon))
    }

    /// The four cofactors of the first row, used by [`Mat4::determinant`].
    fn cofactor_column(&self) -> [f64; 4] {
        let a = self.adjugate();
        [a.m[0], a.m[4], a.m[8], a.m[12]]
    }

    /// The adjugate (transpose of the cofactor matrix) — the numerator of the inverse.
    fn adjugate(&self) -> Mat4 {
        let m = &self.m;
        let mut i = [0.0f64; 16];
        i[0] = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15]
            + m[9] * m[7] * m[14]
            + m[13] * m[6] * m[11]
            - m[13] * m[7] * m[10];
        i[4] = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15]
            - m[8] * m[7] * m[14]
            - m[12] * m[6] * m[11]
            + m[12] * m[7] * m[10];
        i[8] = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15]
            + m[8] * m[7] * m[13]
            + m[12] * m[5] * m[11]
            - m[12] * m[7] * m[9];
        i[12] = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14]
            - m[8] * m[6] * m[13]
            - m[12] * m[5] * m[10]
            + m[12] * m[6] * m[9];
        i[1] = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15]
            - m[9] * m[3] * m[14]
            - m[13] * m[2] * m[11]
            + m[13] * m[3] * m[10];
        i[5] = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15]
            + m[8] * m[3] * m[14]
            + m[12] * m[2] * m[11]
            - m[12] * m[3] * m[10];
        i[9] = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15]
            - m[8] * m[3] * m[13]
            - m[12] * m[1] * m[11]
            + m[12] * m[3] * m[9];
        i[13] = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14]
            + m[8] * m[2] * m[13]
            + m[12] * m[1] * m[10]
            - m[12] * m[2] * m[9];
        i[2] = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15]
            + m[5] * m[3] * m[14]
            + m[13] * m[2] * m[7]
            - m[13] * m[3] * m[6];
        i[6] = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15]
            - m[4] * m[3] * m[14]
            - m[12] * m[2] * m[7]
            + m[12] * m[3] * m[6];
        i[10] = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15]
            + m[4] * m[3] * m[13]
            + m[12] * m[1] * m[7]
            - m[12] * m[3] * m[5];
        i[14] = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14]
            - m[4] * m[2] * m[13]
            - m[12] * m[1] * m[6]
            + m[12] * m[2] * m[5];
        i[3] = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11]
            - m[5] * m[3] * m[10]
            - m[9] * m[2] * m[7]
            + m[9] * m[3] * m[6];
        i[7] = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11]
            + m[4] * m[3] * m[10]
            + m[8] * m[2] * m[7]
            - m[8] * m[3] * m[6];
        i[11] = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11]
            - m[4] * m[3] * m[9]
            - m[8] * m[1] * m[7]
            + m[8] * m[3] * m[5];
        i[15] = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10]
            + m[4] * m[2] * m[9]
            + m[8] * m[1] * m[6]
            - m[8] * m[2] * m[5];
        Mat4 { m: i }
    }
}

/// Recover a rotation from three orthonormal basis columns (Shepperd's method).
fn quat_from_basis(cx: Vec3, cy: Vec3, cz: Vec3) -> Quat {
    // Row-r/col-c of the rotation: column `c` is the c-th basis vector.
    let (m00, m10, m20) = (cx.x, cx.y, cx.z);
    let (m01, m11, m21) = (cy.x, cy.y, cy.z);
    let (m02, m12, m22) = (cz.x, cz.y, cz.z);

    let trace = m00 + m11 + m22;
    if trace > 0.0 {
        let s = math::sqrt(trace + 1.0) * 2.0;
        Quat::new((m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s)
    } else if m00 > m11 && m00 > m22 {
        let s = math::sqrt(1.0 + m00 - m11 - m22) * 2.0;
        Quat::new(0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s)
    } else if m11 > m22 {
        let s = math::sqrt(1.0 + m11 - m00 - m22) * 2.0;
        Quat::new((m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s)
    } else {
        let s = math::sqrt(1.0 + m22 - m00 - m11) * 2.0;
        Quat::new((m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s)
    }
}

/// Matrix product — `self` is applied *after* `o`, matching `Transform::compose` ordering.
impl Mul for Mat4 {
    type Output = Mat4;
    fn mul(self, o: Mat4) -> Mat4 {
        let mut r = Mat4::ZERO;
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += self.m[k * 4 + row] * o.m[col * 4 + k];
                }
                r.m[col * 4 + row] = sum;
            }
        }
        r
    }
}

impl From<Transform> for Mat4 {
    fn from(t: Transform) -> Mat4 {
        Mat4::from_transform(&t)
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

    /// Are two boxes within `epsilon` on both corners?
    pub fn approx_eq(&self, o: &Aabb, epsilon: f64) -> bool {
        self.min.approx_eq(o.min, epsilon) && self.max.approx_eq(o.max, epsilon)
    }

    /// The smallest axis-aligned box enclosing this one after a general affine `m` is applied —
    /// the [`Mat4`] counterpart of [`Aabb::transformed`], valid under shear and mirroring.
    pub fn transformed_affine(&self, m: &Mat4) -> Aabb {
        if self.is_empty() {
            return *self;
        }
        let mut out = Aabb::empty();
        for c in self.corners() {
            out = out.extended_to(m.transform_point(c));
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
    fn mat4_matches_transform_for_trs() {
        let t = Transform::new(
            Vec3::new(3.0, -1.0, 2.0),
            Quat::from_axis_angle(Vec3::new(0.0, 1.0, 1.0), 0.77),
            Vec3::new(2.0, 0.5, 1.25),
        );
        let m = Mat4::from(t);
        let p = Vec3::new(1.5, 4.0, -2.5);
        assert!(m.transform_point(p).approx_eq(t.transform_point(p), 1e-12));
        assert!(m
            .transform_vector(p)
            .approx_eq(t.transform_vector(p), 1e-12));
    }

    /// The reason `Mat4` exists. TRS cannot represent the shear produced by composing a rotation
    /// inside a non-uniform scale, so `Transform::compose` is an approximation there while `Mat4`
    /// multiplication is exact. This pins the boundary so it stays a known, documented limit rather
    /// than a silent wrong answer someone rediscovers later.
    #[test]
    fn mat4_composition_is_exact_where_trs_is_not() {
        let outer = Transform::from_scale(Vec3::new(2.0, 1.0, 1.0)); // non-uniform
        let inner = Transform::from_rotation(Quat::from_axis_angle(Vec3::Z, math::FRAC_PI_4));
        let p = Vec3::new(1.0, 0.0, 0.0);

        let sequential = outer.transform_point(inner.transform_point(p));
        let via_mat4 = (Mat4::from(outer) * Mat4::from(inner)).transform_point(p);
        assert!(
            via_mat4.approx_eq(sequential, 1e-12),
            "Mat4 composition must be exact"
        );

        // And the TRS shortcut genuinely disagrees here — that is the documented limitation.
        let via_trs = Transform {
            translation: outer.transform_point(inner.translation),
            rotation: outer.rotation * inner.rotation,
            scale: outer.scale * inner.scale,
        }
        .transform_point(p);
        assert!(
            !via_trs.approx_eq(sequential, 1e-9),
            "if this now agrees, TRS composition was fixed"
        );

        // The sheared result is correctly reported as un-decomposable.
        assert!((Mat4::from(outer) * Mat4::from(inner))
            .to_transform()
            .is_none());
    }

    #[test]
    fn mat4_composition_matches_trs_when_outer_scale_is_uniform() {
        let outer = Transform::new(
            Vec3::new(1.0, 2.0, 3.0),
            Quat::from_axis_angle(Vec3::Y, 0.5),
            Vec3::splat(2.0), // uniform → compose is exact
        );
        let inner = Transform::new(
            Vec3::new(-1.0, 0.5, 2.0),
            Quat::from_axis_angle(Vec3::Z, 1.1),
            Vec3::new(1.0, 3.0, 0.25),
        );
        let p = Vec3::new(0.5, -1.5, 2.0);
        let via_mat4 = (Mat4::from(outer) * Mat4::from(inner)).transform_point(p);
        assert!(outer
            .compose(&inner)
            .transform_point(p)
            .approx_eq(via_mat4, 1e-10));
    }

    #[test]
    fn mat4_inverse_and_determinant() {
        let m = Mat4::from(Transform::new(
            Vec3::new(3.0, -1.0, 2.0),
            Quat::from_axis_angle(Vec3::new(1.0, 2.0, 3.0), 0.9),
            Vec3::new(2.0, 0.5, 1.25),
        ));
        let inv = m.inverse().expect("invertible");
        assert!((m * inv).approx_eq(&Mat4::IDENTITY, 1e-10));
        // det of a TRS is the product of its scales (rotation contributes 1).
        assert!(math::approx_eq(m.determinant(), 2.0 * 0.5 * 1.25, 1e-10));
        assert!(!m.is_mirroring());
        // Singular matrices report failure rather than producing garbage.
        assert!(Mat4::from_scale(Vec3::new(1.0, 0.0, 1.0))
            .inverse()
            .is_none());
        assert_eq!(Mat4::IDENTITY.determinant(), 1.0);
    }

    #[test]
    fn mat4_reflection_mirrors_and_flips_handedness() {
        // Mirror across the YZ plane: +X becomes -X, other axes untouched.
        let mirror = Mat4::from_reflection(Vec3::X);
        assert!(mirror
            .transform_point(Vec3::new(2.0, 3.0, 4.0))
            .approx_eq(Vec3::new(-2.0, 3.0, 4.0), 1e-12));
        assert!(mirror.is_mirroring(), "a reflection must flip handedness");
        assert!(math::approx_eq(mirror.determinant(), -1.0, 1e-12));
        // Mirroring twice is the identity.
        assert!((mirror * mirror).approx_eq(&Mat4::IDENTITY, 1e-12));
        // No quaternion can do this — which is precisely why Mat4 is needed.
        assert!(!Mat4::from_rotation(Quat::from_axis_angle(Vec3::Z, 1.0)).is_mirroring());
        // An off-origin mirror plane reflects about that plane.
        let plane = Mat4::from_reflection_plane(Vec3::X, Vec3::new(5.0, 0.0, 0.0));
        assert!(plane
            .transform_point(Vec3::new(7.0, 1.0, 1.0))
            .approx_eq(Vec3::new(3.0, 1.0, 1.0), 1e-12));
    }

    #[test]
    fn mat4_decomposition_round_trips() {
        let t = Transform::new(
            Vec3::new(3.0, -1.0, 2.0),
            Quat::from_axis_angle(Vec3::new(0.3, 1.0, -0.5), 1.4),
            Vec3::new(2.0, 3.0, 0.5),
        );
        let back = Mat4::from(t).to_transform().expect("TRS decomposes");
        let p = Vec3::new(1.0, -2.0, 3.0);
        // Compare by effect: quaternion sign is not unique, the transform it encodes is.
        assert!(back
            .transform_point(p)
            .approx_eq(t.transform_point(p), 1e-10));

        // A mirror decomposes too, with the sign folded into the scale.
        let mirrored = Mat4::from_reflection(Vec3::X) * Mat4::from(t);
        let dm = mirrored
            .to_transform()
            .expect("mirror is representable via negative scale");
        assert!(dm.scale.x < 0.0 || dm.scale.y < 0.0 || dm.scale.z < 0.0);
        assert!(dm
            .transform_point(p)
            .approx_eq(mirrored.transform_point(p), 1e-10));
    }

    #[test]
    fn mat4_layout_is_column_major() {
        // Translation lives in the last column (glTF/OpenGL/Three.js layout), i.e. m[12..15].
        let m = Mat4::from_translation(Vec3::new(7.0, 8.0, 9.0));
        assert_eq!([m.m[12], m.m[13], m.m[14]], [7.0, 8.0, 9.0]);
        assert_eq!(m.get(0, 3), 7.0);
        assert_eq!(m.get(3, 3), 1.0);
        assert_eq!(Mat4::IDENTITY.transpose(), Mat4::IDENTITY);
        let s = Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0));
        assert_eq!(s.transpose(), s); // diagonal
    }

    #[test]
    fn aabb_transformed_affine_handles_mirroring() {
        let b = Aabb::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(3.0, 2.0, 2.0));
        let out = b.transformed_affine(&Mat4::from_reflection(Vec3::X));
        let expected = Aabb::new(Vec3::new(-3.0, 0.0, 0.0), Vec3::new(-1.0, 2.0, 2.0));
        assert!(out.approx_eq(&expected, 1e-12), "mirrored box: {out:?}");
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
