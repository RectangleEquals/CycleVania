//! **`CollisionBody`** — a set of collision islands, and the one type four different spatial questions
//! all answer in.
//!
//! `bounds` · `footprint` · `clearance` · `collision` are four *questions*, not four types: the first
//! returns an `Aabb`, the other three return a `CollisionBody`. Keeping them one type is what lets a
//! door reserve its leaf volume and *separately* require its swing room — fusing them would make a
//! developer inflate the footprint to fake the clearance, and worlds go sparse.
//!
//! # Islands, and why the plural is the point
//!
//! ⚠ **A body is a set, not a shape.** A staircase with a landing, an archway, a doorframe: each is one
//! piece of authored content whose collision is genuinely disjoint or genuinely concave. A single-shape
//! body would force every one of those to be approximated by its enclosing box — in the *optimistic*
//! direction, which is the softlock direction.
//!
//! [`CollisionBody::fit_error`] is what makes that approximation *measurable* rather than silent: it
//! reports how much of the body's own bounds the islands do not account for, which is exactly the
//! quantity [`crate::Tolerances`] needs to widen the ambiguity band at a coarse rung.
//!
//! ⚠ **This type replaced `Volume`.** `Volume` was a single box that tried to be all four questions at
//! once; `PlacementNeed` took its intent and `CollisionBody` took its geometry.

use crate::path::ClassPath;
use crate::shape::Shape;
use cv_determinism::{math, Aabb, Transform, Vec3};

/// Which broad-phase layer an island belongs to.
///
/// ⚠ **`Hull` is separate from `Static` on purpose.** The coarse hull built at L2c and the realized
/// static geometry built at L4 both collide; a query that could not tell them apart would silently mix
/// a conservative answer with a firm one and report the result as firm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CollisionLayer {
    /// Coarse bounds standing in for geometry that does not exist yet.
    Hull,
    /// Committed, unmoving geometry.
    #[default]
    Static,
    /// Geometry that moves — an elevator car, a moving platform.
    Dynamic,
}

/// One island: a shape, placed, on a layer, meaning something.
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionData {
    /// What it is, parametrically.
    ///
    /// ⚠ **Never tessellation.** Collision is computed from these parameters; a visual LOD change must
    /// not be able to alter generation.
    pub shape: Shape,
    /// Which broad phase it participates in.
    pub layer: CollisionLayer,
    /// What it *means* to a mechanic — the `Surface` **class**, `None` until one is assigned.
    pub surface: Option<ClassPath>,
    /// Where it sits.
    pub transform: Transform,
}

impl CollisionData {
    /// One island at the origin, static, unsurfaced.
    pub fn new(shape: Shape) -> Self {
        CollisionData {
            shape,
            layer: CollisionLayer::Static,
            surface: None,
            transform: Transform::IDENTITY,
        }
    }

    /// Put it somewhere.
    pub fn at(mut self, translation: Vec3) -> Self {
        self.transform.translation = translation;
        self
    }

    /// Put it on a layer.
    pub fn on(mut self, layer: CollisionLayer) -> Self {
        self.layer = layer;
        self
    }

    /// Give it meaning.
    pub fn meaning(mut self, surface: ClassPath) -> Self {
        self.surface = Some(surface);
        self
    }

    /// This island's world-space bounds.
    ///
    /// ⚠ Translation only, deliberately: a rotated shape's exact bounds cost more than the broad phase
    /// is willing to pay, and the conservative answer here would be the *enclosing* box of the rotated
    /// box — which is what [`Self::bounds_rotated`] gives when a caller genuinely needs it.
    pub fn bounds(&self) -> Aabb {
        let local = self.shape.bounds();
        let s = self.transform.scale;
        let scaled = Aabb::new(
            Vec3::new(local.min.x * s.x, local.min.y * s.y, local.min.z * s.z),
            Vec3::new(local.max.x * s.x, local.max.y * s.y, local.max.z * s.z),
        );
        Aabb::new(
            scaled.min + self.transform.translation,
            scaled.max + self.transform.translation,
        )
    }

    /// The conservative bounds of the rotated island — the enclosing box of the eight rotated corners.
    ///
    /// ⚠ Larger than [`Self::bounds`] and never smaller, which is the safe direction: over-reserving
    /// space makes a world sparse, under-reserving makes it intersect.
    pub fn bounds_rotated(&self) -> Aabb {
        let b = {
            let local = self.shape.bounds();
            let s = self.transform.scale;
            Aabb::new(
                Vec3::new(local.min.x * s.x, local.min.y * s.y, local.min.z * s.z),
                Vec3::new(local.max.x * s.x, local.max.y * s.y, local.max.z * s.z),
            )
        };
        let mut out = Aabb::empty();
        for i in 0..8 {
            let corner = Vec3::new(
                if i & 1 == 0 { b.min.x } else { b.max.x },
                if i & 2 == 0 { b.min.y } else { b.max.y },
                if i & 4 == 0 { b.min.z } else { b.max.z },
            );
            out = out
                .extended_to(self.transform.rotation.rotate(corner) + self.transform.translation);
        }
        out
    }
}

/// A set of collision islands.
///
/// ⚠ **Order is insertion order and stays stable**, because `add`/`remove` are authoring operations a
/// developer performs in a specific sequence and re-sorting them would make the editor's list and the
/// body's contents disagree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollisionBody {
    islands: Vec<CollisionData>,
}

impl CollisionBody {
    /// An empty body — what `clearance()` returns by default.
    ///
    /// ⚠ **Empty is the correct default for clearance, and it is not the same as *unset*.** Most
    /// content requires no free space around it; making the default anything else would reserve room
    /// nothing asked for, everywhere.
    pub fn empty() -> Self {
        CollisionBody::default()
    }

    /// A body of one island.
    pub fn of(shape: Shape) -> Self {
        CollisionBody {
            islands: vec![CollisionData::new(shape)],
        }
    }

    /// Every island, in insertion order.
    pub fn islands(&self) -> &[CollisionData] {
        &self.islands
    }

    /// Add an island.
    pub fn add(&mut self, island: CollisionData) {
        self.islands.push(island);
    }

    /// Add an island, chaining.
    pub fn with(mut self, island: CollisionData) -> Self {
        self.add(island);
        self
    }

    /// Remove the island at an index; `false` if there was none.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.islands.len() {
            return false;
        }
        self.islands.remove(index);
        true
    }

    /// Is this body empty?
    pub fn is_empty(&self) -> bool {
        self.islands.is_empty()
    }

    /// How many islands.
    pub fn len(&self) -> usize {
        self.islands.len()
    }

    /// The union of every island's bounds. Empty for an empty body.
    pub fn bounds(&self) -> Aabb {
        self.islands
            .iter()
            .fold(Aabb::empty(), |acc, i| acc.union(&i.bounds()))
    }

    /// Is this body a single convex island?
    ///
    /// ⚠ **The cheap-path predicate.** A convex body's bounds *are* its shape to within the shape's own
    /// curvature, so a query against one needs no island walk. Anything else must be walked, and
    /// [`Self::fit_error`] says how badly the box would have lied if it had not been.
    pub fn is_convex(&self) -> bool {
        self.islands.len() == 1 && self.islands[0].shape.is_solid()
    }

    /// How much of this body's bounds its islands do not account for, as a fraction in `0..1`.
    ///
    /// A body of one island reports `0.0`; an archway whose box is mostly the gap a player walks
    /// through reports most of it. This is the quantity a coarse rung's ambiguity band is widened by —
    /// a body that fills its box can be decided from the box, one that does not cannot.
    ///
    /// # Why the estimate is deliberately pessimistic
    ///
    /// ⚠ **Error may be over-reported and must never be under-reported.** Under-reporting says a body
    /// is *tighter than it is*, which lets a decision be called firm when the geometry has not settled
    /// — the optimistic direction, which is the softlock direction. Over-reporting only costs a
    /// deferral to the next rung.
    ///
    /// The filled volume is inclusion–exclusion **truncated at pairs**: `Σ islands − Σ pairwise
    /// overlaps`. That is exact for disjoint islands and for one island nested in another, and for
    /// three or more mutually overlapping islands it *under*-counts the union — which over-states the
    /// error, the safe way round. It also stays O(n²) on a handful of islands rather than requiring a
    /// real union.
    ///
    /// ⚠ Measured against island **bounds**, not island volume: a sphere is credited with its whole
    /// box. That direction is unsafe on its own, and it is the reason the pairwise term exists rather
    /// than a bare sum.
    pub fn fit_error(&self) -> f64 {
        let outer = self.bounds();
        if outer.is_empty() {
            return 0.0;
        }
        let total = outer.volume();
        if total <= 0.0 {
            return 0.0;
        }
        let boxes: Vec<Aabb> = self.islands.iter().map(|i| i.bounds()).collect();
        let sum: f64 = boxes.iter().map(Aabb::volume).sum();
        let mut overlap = 0.0;
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                if let Some(x) = a.intersection(b) {
                    overlap += x.volume();
                }
            }
        }
        let filled = math::max(sum - overlap, 0.0);
        let ratio = math::min(filled / total, 1.0);
        1.0 - ratio
    }

    /// Union with another body — how the aggregation default builds an Actor's collision from its
    /// components.
    pub fn union(mut self, other: &CollisionBody) -> Self {
        self.islands.extend(other.islands.iter().cloned());
        self
    }

    /// Does any island's bounds contain this point? A broad-phase answer, not a firm one.
    pub fn may_contain(&self, p: Vec3) -> bool {
        self.islands.iter().any(|i| i.bounds().contains_point(p))
    }

    /// Every surface named by an island, in island order, deduplicated.
    pub fn surfaces(&self) -> Vec<ClassPath> {
        let mut out: Vec<ClassPath> = Vec::new();
        for island in &self.islands {
            if let Some(s) = &island.surface {
                if !out.contains(s) {
                    out.push(s.clone());
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(size: f64) -> Shape {
        Shape::Cube {
            extents: Vec3::new(size, size, size),
            bevel: 0.0,
        }
    }

    #[test]
    fn an_empty_body_is_the_default_and_is_not_a_mistake() {
        // ⚠ Most content requires no free space around it. Any other default reserves room nothing
        // asked for, everywhere.
        let c = CollisionBody::empty();
        assert!(c.is_empty());
        assert!(c.bounds().is_empty());
        assert_eq!(c.fit_error(), 0.0);
    }

    #[test]
    fn a_body_is_a_set_and_the_plural_is_the_point() {
        // An archway: two legs and a lintel. Genuinely three islands, genuinely not one box.
        let arch = CollisionBody::empty()
            .with(CollisionData::new(cube(1.0)).at(Vec3::new(-3.0, 0.0, 0.0)))
            .with(CollisionData::new(cube(1.0)).at(Vec3::new(3.0, 0.0, 0.0)))
            .with(CollisionData::new(cube(1.0)).at(Vec3::new(0.0, 4.0, 0.0)));
        assert_eq!(arch.len(), 3);
        assert!(
            !arch.is_convex(),
            "three disjoint pieces are not one convex shape"
        );
    }

    #[test]
    fn fit_error_measures_what_the_enclosing_box_would_have_lied_by() {
        // ⚠ The whole reason `fit_error` exists: the archway's box is mostly the empty gap a player
        // walks through. A query answered from the box alone would call it solid.
        let solid = CollisionBody::of(cube(2.0));
        assert_eq!(
            solid.fit_error(),
            0.0,
            "one island fills its own box exactly"
        );

        let arch = CollisionBody::empty()
            .with(CollisionData::new(cube(1.0)).at(Vec3::new(-3.0, 0.0, 0.0)))
            .with(CollisionData::new(cube(1.0)).at(Vec3::new(3.0, 0.0, 0.0)));
        assert!(
            arch.fit_error() > 0.5,
            "the gap dominates, got {}",
            arch.fit_error()
        );
    }

    #[test]
    fn a_nested_island_does_not_get_counted_twice() {
        // ⚠ **The bug the pairwise term exists to prevent.** A bare sum credits the small island's
        // volume twice, pushes the ratio past 1, and reports a body as a *perfect* fit for its box —
        // the optimistic direction.
        let hull = CollisionBody::empty()
            .with(CollisionData::new(cube(8.0)))
            .with(CollisionData::new(Shape::Sphere { radius: 1.0 }));
        assert_eq!(
            hull.fit_error(),
            0.0,
            "the big island genuinely fills the box; the small one adds nothing"
        );
    }

    #[test]
    fn error_is_over_reported_rather_than_under_reported_when_the_estimate_is_wrong() {
        // ⚠ Truncating inclusion–exclusion at pairs under-counts the union of three or more
        // overlapping islands, which over-states the error. That is the direction a deferral costs a
        // rung and a wrong *firm* answer costs a softlock.
        let stacked = CollisionBody::empty()
            .with(CollisionData::new(cube(2.0)))
            .with(CollisionData::new(cube(2.0)))
            .with(CollisionData::new(cube(2.0)));
        let e = stacked.fit_error();
        assert!((0.0..=1.0).contains(&e), "always a fraction, got {e}");
        assert!(
            e > 0.0,
            "the true answer is 0.0; erring high is the safe way to be wrong"
        );
    }

    #[test]
    fn a_surface_shape_is_not_a_convex_body() {
        // ⚠ A quad has no interior, so treating it as convex would let a containment test succeed
        // against something with nothing inside it.
        let sheet = CollisionBody::of(Shape::Quad {
            extents: (4.0, 4.0),
        });
        assert!(!sheet.is_convex());
    }

    #[test]
    fn bounds_are_the_union_and_track_placement() {
        let spread = CollisionBody::empty()
            .with(CollisionData::new(cube(2.0)).at(Vec3::new(-5.0, 0.0, 0.0)))
            .with(CollisionData::new(cube(2.0)).at(Vec3::new(5.0, 0.0, 0.0)));
        let b = spread.bounds();
        assert_eq!(b.min.x, -6.0);
        assert_eq!(b.max.x, 6.0);
    }

    #[test]
    fn rotated_bounds_are_conservative_and_never_tighter() {
        // ⚠ Over-reserving makes a world sparse; under-reserving makes it intersect. Only one of those
        // is recoverable.
        let mut d = CollisionData::new(cube(2.0));
        d.transform.rotation =
            cv_determinism::Quat::from_axis_angle(Vec3::Y, std::f64::consts::FRAC_PI_4);
        let axis_aligned = d.bounds();
        let rotated = d.bounds_rotated();
        assert!(
            rotated.volume() >= axis_aligned.volume() - 1e-9,
            "{} < {}",
            rotated.volume(),
            axis_aligned.volume()
        );
    }

    #[test]
    fn layers_stay_apart_so_a_coarse_answer_is_never_reported_as_firm() {
        // ⚠ The L2c hull and the L4 realized geometry both collide. Mixing them would report a
        // conservative answer as a committed one.
        let mixed = CollisionBody::empty()
            .with(CollisionData::new(cube(4.0)).on(CollisionLayer::Hull))
            .with(CollisionData::new(cube(1.0)).on(CollisionLayer::Static));
        let layers: Vec<CollisionLayer> = mixed.islands().iter().map(|i| i.layer).collect();
        assert_eq!(layers, vec![CollisionLayer::Hull, CollisionLayer::Static]);
    }

    #[test]
    fn removing_an_island_out_of_range_reports_it_rather_than_panicking() {
        let mut c = CollisionBody::of(cube(1.0));
        assert!(c.remove(0));
        assert!(!c.remove(0));
        assert!(c.is_empty());
    }

    #[test]
    fn islands_keep_insertion_order_so_the_editor_list_matches_the_body() {
        let a = ClassPath::new("/Content/Surfaces/Stone").unwrap();
        let b = ClassPath::new("/Content/Surfaces/Ice").unwrap();
        let body = CollisionBody::empty()
            .with(CollisionData::new(cube(1.0)).meaning(b.clone()))
            .with(CollisionData::new(cube(1.0)).meaning(a.clone()))
            .with(CollisionData::new(cube(1.0)).meaning(b.clone()));
        assert_eq!(body.surfaces(), vec![b, a], "insertion order, deduplicated");
    }

    #[test]
    fn union_is_how_an_actor_gathers_collision_from_its_components() {
        let a = CollisionBody::of(cube(1.0));
        let b = CollisionBody::of(cube(1.0)).with(CollisionData::new(cube(1.0)));
        assert_eq!(a.union(&b).len(), 3);
    }
}
