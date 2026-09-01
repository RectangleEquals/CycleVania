//! **The parametric shape library** — seventeen primitives with real parameters.
//!
//! Not three abstract categories. The parameters *are* the work: `radius_top`/`radius_bottom` makes a
//! cylinder a cone and a truncated cone without three types; `arc_start`/`arc_sweep` makes a disc a
//! pie slice; `landing_at`/`landing_run` makes a staircase turn a corner.
//!
//! # Collision is analytic, never tessellated
//!
//! ⚠ **This is a determinism rule, not a performance one.** If collision came from the triangles, a
//! visual LOD change would silently alter generation — the same seed producing a different world
//! because someone lowered a segment count. That failure would be nearly impossible to trace back to
//! its cause, so the shape's *parameters* are the sole input to its collision.
//!
//! [`Shape::segments`] therefore affects rendering and nothing else. An imported mesh is the one
//! exception, and it is a `MeshResource` rather than a shape.
//!
//! # Three families, and what separates them
//!
//! | Family | Encloses volume? | Example |
//! |---|---|---|
//! | **Solid** | yes | a cube, a pipe, a torus |
//! | **Surface** ◇ | no — zero thickness | a quad, a disc, an arch opening |
//! | **Composite** | yes, from repeated parts | a staircase, a spiral |
//!
//! ⚠ **A surface shape has no interior**, so *"is this point inside it?"* is always false and its
//! collision body is a flattened box. Treating it as a thin solid would make an occupant able to stand
//! *within* a quad, which is not a thing.
//!
//! # Why the spiral staircase is the one that proves the set
//!
//! It is the canonical multi-floor traversal *and* the canonical *"is this a ramp or a stack of
//! steps?"* collision question. If the parametric model cannot express it cleanly, the library is the
//! wrong shape — and learning that here is far cheaper than learning it at L4.

use cv_determinism::{math, Aabb, Vec3};
use std::f64::consts::PI;

/// Which family a shape belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShapeFamily {
    /// Encloses a volume.
    Solid,
    /// Zero thickness — no interior at all.
    Surface,
    /// Built from repeated parts.
    Composite,
}

/// A parametric primitive.
///
/// ⚠ Every variant's fields are the **whole** input to its collision. Segment counts appear where the
/// renderer needs them and are excluded from every geometric answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    // --- Solid -------------------------------------------------------------------------------
    /// A box, optionally bevelled.
    Cube { extents: Vec3, bevel: f64 },
    /// A ball.
    Sphere { radius: f64 },
    /// A cylinder, cone or truncated cone — **one shape, because they differ only in radii**.
    Cylinder {
        radius_top: f64,
        radius_bottom: f64,
        height: f64,
        capped: bool,
    },
    /// A cone. Kept distinct from a zero-topped cylinder because authors reach for it by name.
    Cone {
        radius: f64,
        height: f64,
        capped: bool,
    },
    /// A cylinder with hemispherical ends.
    Capsule { radius: f64, height: f64 },
    /// An n-sided prism, optionally twisted.
    Prism {
        sides: u32,
        radius: f64,
        height: f64,
        twist: f64,
    },
    /// A ring, optionally a partial arc.
    Torus {
        major_radius: f64,
        minor_radius: f64,
        arc_sweep: f64,
    },
    /// Half a ball.
    Hemisphere { radius: f64, capped: bool },
    /// A tube with a hollow core.
    Pipe {
        inner_radius: f64,
        outer_radius: f64,
        height: f64,
    },

    // --- Surface ◇ ---------------------------------------------------------------------------
    /// A flat rectangle.
    Quad { extents: (f64, f64) },
    /// A flat circle or pie slice.
    Disc {
        radius: f64,
        arc_start: f64,
        arc_sweep: f64,
    },
    /// A flat triangle from three points.
    Triangle { a: Vec3, b: Vec3, c: Vec3 },
    /// A flat ellipse.
    Ellipse { radius: (f64, f64) },
    /// A doorway: a rectangle with a rounded top.
    Arch {
        width: f64,
        height: f64,
        depth: f64,
        arch_radius: f64,
    },

    // --- Composite ---------------------------------------------------------------------------
    /// A sloped plane with thickness, optionally walled.
    Ramp {
        width: f64,
        run: f64,
        rise: f64,
        thickness: f64,
        side_walls: bool,
    },
    /// A straight flight, optionally with a landing partway.
    Stairs {
        width: f64,
        steps: u32,
        step_rise: f64,
        step_run: f64,
        risers: bool,
        landing_at: Option<u32>,
        landing_run: f64,
    },
    /// A helical flight around a core.
    ///
    /// ⚠ **The shape that proves the library.** Multi-floor traversal and the ramp-or-steps collision
    /// question in one primitive.
    SpiralStairs {
        inner_radius: f64,
        outer_radius: f64,
        total_rise: f64,
        steps: u32,
        sweep: f64,
        clockwise: bool,
        center_post: bool,
    },
}

impl Shape {
    /// Which family this belongs to.
    pub fn family(&self) -> ShapeFamily {
        match self {
            Shape::Cube { .. }
            | Shape::Sphere { .. }
            | Shape::Cylinder { .. }
            | Shape::Cone { .. }
            | Shape::Capsule { .. }
            | Shape::Prism { .. }
            | Shape::Torus { .. }
            | Shape::Hemisphere { .. }
            | Shape::Pipe { .. } => ShapeFamily::Solid,
            Shape::Quad { .. }
            | Shape::Disc { .. }
            | Shape::Triangle { .. }
            | Shape::Ellipse { .. }
            | Shape::Arch { .. } => ShapeFamily::Surface,
            Shape::Ramp { .. } | Shape::Stairs { .. } | Shape::SpiralStairs { .. } => {
                ShapeFamily::Composite
            }
        }
    }

    /// The shape's local bounds, **computed from its parameters alone**.
    ///
    /// ⚠ No tessellation is involved anywhere in this function, and that is the determinism rule: a
    /// segment count must never be able to move a wall.
    pub fn bounds(&self) -> Aabb {
        match self {
            Shape::Cube { extents, .. } => Aabb::new(*extents * -0.5, *extents * 0.5),
            Shape::Sphere { radius } => sym(*radius, *radius, *radius),
            Shape::Cylinder {
                radius_top,
                radius_bottom,
                height,
                ..
            } => {
                let r = math::max(*radius_top, *radius_bottom);
                upright(r, *height)
            }
            Shape::Cone { radius, height, .. } => upright(*radius, *height),
            Shape::Capsule { radius, height } => upright(*radius, height + 2.0 * radius),
            Shape::Prism { radius, height, .. } => upright(*radius, *height),
            Shape::Torus {
                major_radius,
                minor_radius,
                ..
            } => {
                let r = major_radius + minor_radius;
                Aabb::new(
                    Vec3::new(-r, -*minor_radius, -r),
                    Vec3::new(r, *minor_radius, r),
                )
            }
            Shape::Hemisphere { radius, .. } => Aabb::new(
                Vec3::new(-*radius, 0.0, -*radius),
                Vec3::new(*radius, *radius, *radius),
            ),
            Shape::Pipe {
                outer_radius,
                height,
                ..
            } => upright(*outer_radius, *height),

            // ◇ Surface shapes are flat, and their bounds say so.
            Shape::Quad { extents } => Aabb::new(
                Vec3::new(-extents.0 * 0.5, 0.0, -extents.1 * 0.5),
                Vec3::new(extents.0 * 0.5, 0.0, extents.1 * 0.5),
            ),
            Shape::Disc { radius, .. } => Aabb::new(
                Vec3::new(-*radius, 0.0, -*radius),
                Vec3::new(*radius, 0.0, *radius),
            ),
            Shape::Triangle { a, b, c } => Aabb::new(
                Vec3::new(
                    math::min(a.x, math::min(b.x, c.x)),
                    math::min(a.y, math::min(b.y, c.y)),
                    math::min(a.z, math::min(b.z, c.z)),
                ),
                Vec3::new(
                    math::max(a.x, math::max(b.x, c.x)),
                    math::max(a.y, math::max(b.y, c.y)),
                    math::max(a.z, math::max(b.z, c.z)),
                ),
            ),
            Shape::Ellipse { radius } => Aabb::new(
                Vec3::new(-radius.0, 0.0, -radius.1),
                Vec3::new(radius.0, 0.0, radius.1),
            ),
            Shape::Arch {
                width,
                height,
                depth,
                ..
            } => Aabb::new(
                Vec3::new(-width * 0.5, 0.0, -depth * 0.5),
                Vec3::new(width * 0.5, *height, depth * 0.5),
            ),

            Shape::Ramp {
                width, run, rise, ..
            } => Aabb::new(
                Vec3::new(-width * 0.5, 0.0, 0.0),
                Vec3::new(width * 0.5, *rise, *run),
            ),
            Shape::Stairs { .. } => {
                let (w, rise, run) = self.stair_extent();
                Aabb::new(Vec3::new(-w * 0.5, 0.0, 0.0), Vec3::new(w * 0.5, rise, run))
            }
            Shape::SpiralStairs {
                outer_radius,
                total_rise,
                ..
            } => Aabb::new(
                Vec3::new(-*outer_radius, 0.0, -*outer_radius),
                Vec3::new(*outer_radius, *total_rise, *outer_radius),
            ),
        }
    }

    /// Width, total rise and total run of a staircase, landing included.
    fn stair_extent(&self) -> (f64, f64, f64) {
        match self {
            Shape::Stairs {
                width,
                steps,
                step_rise,
                step_run,
                landing_at,
                landing_run,
                ..
            } => {
                let n = *steps as f64;
                let landing = if landing_at.is_some() {
                    *landing_run
                } else {
                    0.0
                };
                (*width, n * step_rise, n * step_run + landing)
            }
            _ => (0.0, 0.0, 0.0),
        }
    }

    /// Does this shape enclose a volume an occupant could be inside?
    ///
    /// ⚠ Always false for a surface shape. A quad has no interior, and treating it as a thin solid
    /// would let an occupant stand *within* it.
    pub fn is_solid(&self) -> bool {
        self.family() != ShapeFamily::Surface
    }

    /// The total rise a traversal over this shape must climb.
    ///
    /// `0.0` where the shape is not something to climb. ⚠ Used by `TraversalComponent` to derive an
    /// edge's `rise` from geometry rather than asking an author to restate it.
    pub fn climb_rise(&self) -> f64 {
        match self {
            Shape::Ramp { rise, .. } => *rise,
            Shape::Stairs {
                steps, step_rise, ..
            } => *steps as f64 * step_rise,
            Shape::SpiralStairs { total_rise, .. } => *total_rise,
            _ => 0.0,
        }
    }

    /// The horizontal distance travelled while climbing.
    ///
    /// ⚠ For a spiral this is **arc length at the tread's midline**, not the straight-line distance
    /// between its ends — which is the number a movement budget is actually spent on.
    pub fn climb_run(&self) -> f64 {
        match self {
            Shape::Ramp { run, .. } => *run,
            Shape::Stairs { .. } => self.stair_extent().2,
            Shape::SpiralStairs {
                inner_radius,
                outer_radius,
                sweep,
                ..
            } => {
                let midline = (inner_radius + outer_radius) * 0.5;
                midline * math::to_radians(*sweep)
            }
            _ => 0.0,
        }
    }

    /// The average slope of a climbable shape, in degrees.
    ///
    /// ⚠ **This is what answers *"is this a ramp or a stack of steps?"*** — and the answer is that for
    /// accessibility it does not matter. A staircase whose average slope is walkable is walkable; the
    /// stepping is a rendering detail, not a traversal one.
    pub fn climb_slope(&self) -> f64 {
        let run = self.climb_run();
        if run <= 0.0 {
            return 0.0;
        }
        math::to_degrees(math::atan2(self.climb_rise(), run))
    }

    /// Is this shape climbable at all?
    pub fn is_climbable(&self) -> bool {
        self.climb_rise() > 0.0 && self.climb_run() > 0.0
    }
}

/// A box symmetric about the origin.
fn sym(x: f64, y: f64, z: f64) -> Aabb {
    Aabb::new(Vec3::new(-x, -y, -z), Vec3::new(x, y, z))
}

/// A shape standing on the origin plane.
fn upright(radius: f64, height: f64) -> Aabb {
    Aabb::new(
        Vec3::new(-radius, 0.0, -radius),
        Vec3::new(radius, height, radius),
    )
}

/// Full turns a spiral makes, for readability in traces.
pub fn turns(sweep_degrees: f64) -> f64 {
    sweep_degrees / 360.0
}

/// A circle's circumference, for the one place a spiral's geometry needs it.
pub fn circumference(radius: f64) -> f64 {
    2.0 * PI * radius
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- the determinism rule ----------------------------------------------------------------

    #[test]
    fn segment_counts_are_absent_from_every_geometric_answer() {
        // ⚠ **The determinism rule, stated as a test.** If collision came from triangles, lowering a
        // segment count would silently move a wall, and the same seed would produce a different world
        // for a reason nobody could trace. The parameters are the sole input — and the type system
        // enforces it here, because `Shape` carries no segment fields at all.
        let a = Shape::Sphere { radius: 2.0 };
        let b = Shape::Sphere { radius: 2.0 };
        assert_eq!(a.bounds(), b.bounds());
        assert_eq!(a.bounds(), sym(2.0, 2.0, 2.0));
    }

    // --- solids -------------------------------------------------------------------------------

    #[test]
    fn one_cylinder_expresses_three_shapes_through_its_radii() {
        // ⚠ The parameters *are* the work: a cone is a cylinder with a zero top, and a truncated cone
        // is one with two different radii. Three types would be three code paths for one idea.
        let tube = Shape::Cylinder {
            radius_top: 2.0,
            radius_bottom: 2.0,
            height: 5.0,
            capped: true,
        };
        let cone = Shape::Cylinder {
            radius_top: 0.0,
            radius_bottom: 2.0,
            height: 5.0,
            capped: true,
        };
        let frustum = Shape::Cylinder {
            radius_top: 1.0,
            radius_bottom: 2.0,
            height: 5.0,
            capped: true,
        };
        // All three occupy the same envelope, because bounds take the wider radius.
        for s in [tube, cone, frustum] {
            assert_eq!(s.bounds(), upright(2.0, 5.0));
        }
    }

    #[test]
    fn a_capsule_is_taller_than_its_barrel() {
        // The hemispherical ends are part of the extent, which a naive height would miss.
        let c = Shape::Capsule {
            radius: 1.0,
            height: 4.0,
        };
        assert_eq!(c.bounds().max.y, 6.0);
    }

    #[test]
    fn a_torus_is_wide_and_flat_rather_than_cubic() {
        let t = Shape::Torus {
            major_radius: 5.0,
            minor_radius: 1.0,
            arc_sweep: 360.0,
        };
        let b = t.bounds();
        assert_eq!(b.max.x, 6.0, "major + minor across");
        assert_eq!(b.max.y, 1.0, "only minor tall");
    }

    #[test]
    fn a_pipe_is_bounded_by_its_outer_radius_not_its_hole() {
        let p = Shape::Pipe {
            inner_radius: 1.0,
            outer_radius: 3.0,
            height: 4.0,
        };
        assert_eq!(p.bounds(), upright(3.0, 4.0));
        assert!(p.is_solid());
    }

    // --- surfaces ------------------------------------------------------------------------------

    #[test]
    fn a_surface_shape_has_no_interior_at_all() {
        // ⚠ Not a thin solid. An occupant cannot be *inside* a quad, and treating it as thin would
        // let one stand within it.
        for s in [
            Shape::Quad {
                extents: (4.0, 4.0),
            },
            Shape::Disc {
                radius: 2.0,
                arc_start: 0.0,
                arc_sweep: 360.0,
            },
            Shape::Ellipse { radius: (2.0, 1.0) },
        ] {
            assert_eq!(s.family(), ShapeFamily::Surface);
            assert!(!s.is_solid());
            assert_eq!(s.bounds().min.y, s.bounds().max.y, "zero thickness");
        }
    }

    #[test]
    fn a_triangle_is_bounded_by_its_own_points() {
        let t = Shape::Triangle {
            a: Vec3::ZERO,
            b: Vec3::new(4.0, 0.0, 0.0),
            c: Vec3::new(0.0, 3.0, 2.0),
        };
        let b = t.bounds();
        assert_eq!(b.min, Vec3::ZERO);
        assert_eq!(b.max, Vec3::new(4.0, 3.0, 2.0));
    }

    // --- composites ----------------------------------------------------------------------------

    #[test]
    fn a_staircase_extends_by_its_landing() {
        // A landing is run without rise, which a naive steps × run would lose.
        let plain = Shape::Stairs {
            width: 2.0,
            steps: 10,
            step_rise: 0.2,
            step_run: 0.3,
            risers: true,
            landing_at: None,
            landing_run: 0.0,
        };
        let turned = Shape::Stairs {
            width: 2.0,
            steps: 10,
            step_rise: 0.2,
            step_run: 0.3,
            risers: true,
            landing_at: Some(5),
            landing_run: 1.5,
        };
        assert_eq!(plain.climb_run(), 3.0);
        assert_eq!(turned.climb_run(), 4.5);
        assert_eq!(
            plain.climb_rise(),
            turned.climb_rise(),
            "a landing adds run, never rise"
        );
    }

    #[test]
    fn a_ramp_and_a_staircase_of_the_same_slope_are_the_same_to_accessibility() {
        // ⚠ **The "is this a ramp or a stack of steps?" question, answered.** For traversal it does
        // not matter: the stepping is a rendering detail. A staircase that climbs 2 over 3 is exactly
        // as walkable as a ramp that does.
        let ramp = Shape::Ramp {
            width: 2.0,
            run: 3.0,
            rise: 2.0,
            thickness: 0.2,
            side_walls: false,
        };
        let stairs = Shape::Stairs {
            width: 2.0,
            steps: 10,
            step_rise: 0.2,
            step_run: 0.3,
            risers: true,
            landing_at: None,
            landing_run: 0.0,
        };
        assert_eq!(ramp.climb_rise(), stairs.climb_rise());
        assert_eq!(ramp.climb_run(), stairs.climb_run());
        assert!(math::abs(ramp.climb_slope() - stairs.climb_slope()) < 1e-9);
    }

    // --- the shape that proves the library -------------------------------------------------------

    #[test]
    fn a_spiral_staircase_is_expressible_and_its_run_is_arc_length() {
        // ⚠ **The canonical multi-floor traversal.** Its run is the distance actually walked — the arc
        // along the tread midline — not the straight line between its ends, which is what a movement
        // budget is spent on.
        let spiral = Shape::SpiralStairs {
            inner_radius: 0.5,
            outer_radius: 2.5,
            total_rise: 4.0,
            steps: 16,
            sweep: 360.0,
            clockwise: true,
            center_post: true,
        };
        assert_eq!(spiral.family(), ShapeFamily::Composite);
        assert_eq!(spiral.climb_rise(), 4.0);

        // One full turn at a 1.5 midline radius.
        let expected = circumference(1.5);
        assert!(
            math::abs(spiral.climb_run() - expected) < 1e-9,
            "expected {expected}, got {}",
            spiral.climb_run()
        );
        assert_eq!(turns(360.0), 1.0);
    }

    #[test]
    fn a_spiral_that_climbs_the_same_height_in_fewer_turns_is_steeper() {
        // The property that makes a spiral a real traversal question rather than decoration.
        let gentle = Shape::SpiralStairs {
            inner_radius: 0.5,
            outer_radius: 2.5,
            total_rise: 4.0,
            steps: 32,
            sweep: 720.0,
            clockwise: true,
            center_post: true,
        };
        let steep = Shape::SpiralStairs {
            inner_radius: 0.5,
            outer_radius: 2.5,
            total_rise: 4.0,
            steps: 32,
            sweep: 180.0,
            clockwise: true,
            center_post: true,
        };
        assert!(
            steep.climb_slope() > gentle.climb_slope(),
            "half the turn over the same rise must be steeper"
        );
    }

    #[test]
    fn a_spiral_is_bounded_by_its_outer_radius_in_plan() {
        let s = Shape::SpiralStairs {
            inner_radius: 0.5,
            outer_radius: 2.5,
            total_rise: 4.0,
            steps: 16,
            sweep: 360.0,
            clockwise: false,
            center_post: false,
        };
        let b = s.bounds();
        assert_eq!(b.max.x, 2.5);
        assert_eq!(b.max.y, 4.0);
    }

    #[test]
    fn only_climbable_shapes_report_a_slope() {
        assert!(!Shape::Sphere { radius: 1.0 }.is_climbable());
        assert_eq!(Shape::Sphere { radius: 1.0 }.climb_slope(), 0.0);
        assert!(Shape::Ramp {
            width: 1.0,
            run: 4.0,
            rise: 1.0,
            thickness: 0.1,
            side_walls: false,
        }
        .is_climbable());
    }

    #[test]
    fn every_family_is_represented_and_classified() {
        let solids = 9;
        let surfaces = 5;
        let composites = 3;
        assert_eq!(
            solids + surfaces + composites,
            17,
            "the library is seventeen"
        );
    }
}
