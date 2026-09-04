//! **L2a–L2c: floors first, then the bounds derived from them.**
//!
//! # The chicken-and-egg this dissolves
//!
//! Content hooks need spatial answers — *"is there a ledge within 30 metres?"* — and spatial answers
//! need geometry, which is expensive to build and which the hooks' own answers are supposed to shape.
//! Earlier designs tried to break the loop by committing **walls** first, and could not: walls are
//! exactly the part that has to move when a hook asks for something.
//!
//! > **Floors do not depend on hulls; hulls depend on floors.**
//!
//! Standable floor is *projected* early and cheaply. Bounds are derived from it. By L2c a `requires()`
//! or `judge()` can ask a real spatial question, with L3 and L4 **refining** the answer rather than
//! producing one for the first time.
//!
//! | Step | Produces | Cost |
//! |---|---|---|
//! | **L2a** | floor collision — committed content collision plus projected standable geometry | cheap |
//! | **L2b** | the per-scope AABB envelope | trivial |
//! | **L2c** | the **dual bounds**: inner solid, outer hull. Spatial queries go live | O(n log n) |
//!
//! # Why the answer comes in two bounds rather than one
//!
//! A single approximation has to choose between lying optimistically and lying pessimistically. Two
//! bounds do not have to choose:
//!
//! * **inner** — the union of standable volumes actually committed. Everything here **is** solid.
//! * **outer** — the convex hull of the same floors. Everything real is inside it; parts of it are not
//!   real.
//!
//! A point inside the inner bound is definitely reachable-in-principle; a point outside the outer bound
//! is definitely not; between them the honest answer is *maybe*, which is what [`crate::geometry`]'s
//! query layer turns into a `Trivalent` rather than a guess.
//!
//! ⚠ **The gap is deliberately optimistic, and that is the softlock direction.** An L-shaped room's hull
//! fills in the notch, so a sightline reads *clear* at L2 and *blocked* at L4. Erring the other way
//! would let the solver rule out routes that turn out to exist, and a route wrongly ruled **out** can
//! strand a player where a route wrongly ruled **in** merely gets re-checked.
//!
//! # Monotone by ordering, not by machinery
//!
//! ```text
//! AABB  ⊇  convex hull  ⊇  mildly concave  ⊇  realized
//! ```
//!
//! Each step only ever **tightens**, so error bounds only ever **shrink**. That is where *"a decision
//! made outside the band at L2 cannot be overturned at L4"* comes from — the ordering itself, not an
//! extra proof obligation carried alongside it.

use crate::arena::Handle;
use crate::geometry::{CoarseGeometry, Face};
use crate::node::Node;
use crate::object::ObjectId;
use cv_determinism::{math, Aabb, Vec3};
use std::collections::BTreeMap;

/// Up, for floor purposes. Y-up matches [`Face::PosY`] being the top of a solid box.
const UP: Vec3 = Vec3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};

/// A standable patch of collision — one face that an occupant could be supported by.
///
/// ⚠ **Detection is purely geometric.** Nothing a `Surface` says may remove a region: lava is a floor
/// with a restrictive `supports()`, present in the graph and gated. If a surface could veto detection,
/// hover boots could never create a route across it, because there would be nothing to route *through*
/// (P2 — gate a region, never delete it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloorSurface {
    /// What owns the collider this face belongs to.
    pub owner: ObjectId,
    /// The face's own extent, flattened to zero thickness along its axis.
    pub patch: Aabb,
    /// Degrees away from level. `0.0` is flat; the detector's cutoff is the project's slope limit.
    pub slope: f64,
}

impl FloorSurface {
    /// The volume an occupant of this height would stand in, above the patch.
    ///
    /// This is what the inner bound is built from: floor is a *surface*, but standing needs a
    /// *volume*, and the volume is what a query can be asked to contain.
    pub fn standing_volume(&self, standing_height: f64) -> Aabb {
        let mut max = self.patch.max;
        max.y = self.patch.max.y + standing_height;
        Aabb::new(self.patch.min, max)
    }
}

/// **L2a** — every standable face in the geometry.
///
/// A face qualifies when its outward normal is within `max_slope_degrees` of up. For axis-aligned
/// colliders that selects exactly the top faces; the test is written against the normal rather than the
/// face so it keeps meaning when L4 replaces boxes with triangles.
///
/// ⚠ **Order is deterministic** — collider order, then [`Face::ALL`] order — because floors feed the
/// bounds, the bounds feed queries, and query results reach the output.
pub fn detect_floors(geometry: &CoarseGeometry, max_slope_degrees: f64) -> Vec<FloorSurface> {
    let cutoff = math::cos(math::to_radians(max_slope_degrees.clamp(0.0, 90.0)));
    let mut out = Vec::new();
    for collider in geometry.colliders() {
        for face in Face::ALL {
            let alignment = face.normal().dot(UP);
            if alignment < cutoff {
                continue;
            }
            out.push(FloorSurface {
                owner: collider.owner,
                patch: face_patch(&collider.bounds, face),
                slope: math::to_degrees(math::acos(alignment.clamp(-1.0, 1.0))),
            });
        }
    }
    out
}

/// The face of a box, flattened to zero thickness along its own axis.
fn face_patch(b: &Aabb, face: Face) -> Aabb {
    let (mut min, mut max) = (b.min, b.max);
    let axis = face.axis();
    let positive = face.normal().dot(Vec3::new(1.0, 1.0, 1.0)) > 0.0;
    let plane = match axis {
        0 => {
            if positive {
                b.max.x
            } else {
                b.min.x
            }
        }
        1 => {
            if positive {
                b.max.y
            } else {
                b.min.y
            }
        }
        _ => {
            if positive {
                b.max.z
            } else {
                b.min.z
            }
        }
    };
    match axis {
        0 => {
            min.x = plane;
            max.x = plane;
        }
        1 => {
            min.y = plane;
            max.y = plane;
        }
        _ => {
            min.z = plane;
            max.z = plane;
        }
    }
    Aabb::new(min, max)
}

/// The dual bounds for one scope — **L2b and L2c**.
///
/// Built from floors, never from walls, and the ordering `envelope ⊇ outer ⊇ inner` is an invariant
/// this type maintains rather than a property a caller must remember.
#[derive(Clone, Debug, PartialEq)]
pub struct ScopeBounds {
    /// **L2b** — the axis-aligned envelope. Trivial to build, and the loosest of the three.
    envelope: Aabb,
    /// **L2c inner** — standing volumes over committed floors. Everything here is solid.
    inner: Vec<Aabb>,
    /// **L2c outer** — the convex hull of the same floors. Everything real is inside it.
    outer: ConvexHull,
}

impl ScopeBounds {
    /// Derive the bounds for a set of floors.
    ///
    /// Empty floors give empty bounds that contain nothing — a scope with nowhere to stand is a real
    /// answer, not a degenerate case to guard against at every call site.
    pub fn from_floors(floors: &[FloorSurface], standing_height: f64) -> Self {
        let volumes: Vec<Aabb> = floors
            .iter()
            .map(|f| f.standing_volume(standing_height))
            .collect();
        let envelope = volumes
            .iter()
            .copied()
            .reduce(|a, b| a.union(&b))
            .unwrap_or(Aabb::new(Vec3::ZERO, Vec3::ZERO));
        let points: Vec<Vec3> = volumes.iter().flat_map(|v| v.corners()).collect();
        ScopeBounds {
            envelope,
            inner: volumes,
            outer: ConvexHull::of(&points),
        }
    }

    /// The L2b envelope.
    pub fn envelope(&self) -> Aabb {
        self.envelope
    }

    /// Is this point **definitely** inside something standable?
    pub fn inner_contains(&self, p: Vec3) -> bool {
        self.inner.iter().any(|v| v.contains_point(p))
    }

    /// Is this point inside the optimistic outer bound?
    ///
    /// `false` here is a **definite no** — the property the whole ladder exists to provide.
    pub fn outer_contains(&self, p: Vec3) -> bool {
        self.outer.contains(p)
    }

    /// Nothing to stand on anywhere in this scope.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// A convex hull in three dimensions, as the half-spaces bounding it.
///
/// ⚠ **Faces, not vertices, because the only question asked of it is `contains`.** Storing the vertex
/// set would mean re-deriving the faces on every query, and the query runs far more often than the
/// build.
///
/// Built by incremental insertion: seed a tetrahedron, then for each remaining point outside the
/// current hull, drop every face it can see and re-close the gap. Deterministic, and cheap enough at
/// the point counts a scope produces.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ConvexHull {
    /// `(normal, offset)` with the interior at `normal · p <= offset`.
    planes: Vec<(Vec3, f64)>,
}

/// Below this, two points are the same point and a plane is degenerate.
const EPS: f64 = 1e-9;

impl ConvexHull {
    /// The hull of a point set. Fewer than four non-coplanar points gives an empty hull.
    pub fn of(points: &[Vec3]) -> Self {
        let Some(seed) = seed_tetrahedron(points) else {
            return ConvexHull::default();
        };
        // Faces as vertex triples, wound so the normal points outward from the interior point.
        let centre = (seed.0 + seed.1 + seed.2 + seed.3) * 0.25;
        let mut faces: Vec<[Vec3; 3]> = vec![
            [seed.0, seed.1, seed.2],
            [seed.0, seed.1, seed.3],
            [seed.0, seed.2, seed.3],
            [seed.1, seed.2, seed.3],
        ];
        for f in &mut faces {
            if outward_normal(f, centre).is_none() {
                return ConvexHull::default();
            }
        }

        for &p in points {
            // Every face this point can see is no longer on the hull.
            let mut visible = Vec::new();
            for (i, f) in faces.iter().enumerate() {
                if let Some((n, d)) = outward_normal(f, centre) {
                    if n.dot(p) - d > EPS {
                        visible.push(i);
                    }
                }
            }
            if visible.is_empty() {
                continue;
            }
            // The horizon is every edge belonging to exactly one visible face.
            let mut edges: Vec<(Vec3, Vec3)> = Vec::new();
            for &i in &visible {
                let f = faces[i];
                for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                    match edges.iter().position(|e| same_edge(*e, (a, b))) {
                        Some(k) => {
                            edges.remove(k);
                        }
                        None => edges.push((a, b)),
                    }
                }
            }
            for i in visible.into_iter().rev() {
                faces.remove(i);
            }
            for (a, b) in edges {
                faces.push([a, b, p]);
            }
        }

        let mut planes = Vec::with_capacity(faces.len());
        for f in &faces {
            if let Some(pl) = outward_normal(f, centre) {
                planes.push(pl);
            }
        }
        ConvexHull { planes }
    }

    /// Is the point inside, or on, the hull?
    pub fn contains(&self, p: Vec3) -> bool {
        !self.planes.is_empty() && self.planes.iter().all(|(n, d)| n.dot(p) - d <= EPS)
    }

    /// How many bounding planes the hull has. Zero means degenerate — coplanar or too few points.
    pub fn plane_count(&self) -> usize {
        self.planes.len()
    }
}

/// Four points that are not coplanar, or `None`.
fn seed_tetrahedron(points: &[Vec3]) -> Option<(Vec3, Vec3, Vec3, Vec3)> {
    let a = *points.first()?;
    let b = *points.iter().find(|p| (**p - a).length() > EPS)?;
    let c = *points
        .iter()
        .find(|p| (**p - a).cross(**p - b).length() > EPS)?;
    let n = (b - a).cross(c - a);
    let d = *points.iter().find(|p| math::abs(n.dot(**p - a)) > EPS)?;
    Some((a, b, c, d))
}

/// The outward plane of a face, given a point known to be inside.
fn outward_normal(f: &[Vec3; 3], interior: Vec3) -> Option<(Vec3, f64)> {
    let n = (f[1] - f[0]).cross(f[2] - f[0]);
    if n.length() <= EPS {
        return None;
    }
    let n = n.normalized();
    let d = n.dot(f[0]);
    if n.dot(interior) - d > 0.0 {
        Some((n * -1.0, -d))
    } else {
        Some((n, d))
    }
}

/// Edges match regardless of direction — the horizon does not care which way a face wound.
fn same_edge(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    let close = |p: Vec3, q: Vec3| (p - q).length() <= EPS;
    (close(a.0, b.0) && close(a.1, b.1)) || (close(a.0, b.1) && close(a.1, b.0))
}

/// The bounds for every scope that has any floor — **the L2a→L2c pass**, run once.
///
/// ⚠ **Keyed by scope, because that is what a hook asks about.** A query arriving at L2d says *"in this
/// Space"*, not *"in this collider"*, so the pass groups by [`Collider::scope`] and a collider with no
/// scope contributes to nothing. That is deliberate: geometry the graph does not own has no scope to
/// answer for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FloorLadder {
    scopes: BTreeMap<Handle<Node>, ScopeBounds>,
    floors: BTreeMap<Handle<Node>, Vec<FloorSurface>>,
}

impl FloorLadder {
    /// Run L2a (detect), L2b (envelope) and L2c (dual bounds) for every scope in the geometry.
    ///
    /// `max_slope_degrees` and `standing_height` come from project settings — they are the two numbers
    /// that decide what counts as somewhere an occupant could be.
    pub fn build(geometry: &CoarseGeometry, max_slope_degrees: f64, standing_height: f64) -> Self {
        let mut floors: BTreeMap<Handle<Node>, Vec<FloorSurface>> = BTreeMap::new();
        let cutoff = math::cos(math::to_radians(max_slope_degrees.clamp(0.0, 90.0)));
        for collider in geometry.colliders() {
            let Some(scope) = collider.scope else {
                continue;
            };
            for face in Face::ALL {
                let alignment = face.normal().dot(UP);
                if alignment < cutoff {
                    continue;
                }
                floors.entry(scope).or_default().push(FloorSurface {
                    owner: collider.owner,
                    patch: face_patch(&collider.bounds, face),
                    slope: math::to_degrees(math::acos(alignment.clamp(-1.0, 1.0))),
                });
            }
        }
        let scopes = floors
            .iter()
            .map(|(k, v)| (*k, ScopeBounds::from_floors(v, standing_height)))
            .collect();
        FloorLadder { scopes, floors }
    }

    /// The bounds for one scope, if it has any floor at all.
    pub fn bounds(&self, scope: Handle<Node>) -> Option<&ScopeBounds> {
        self.scopes.get(&scope)
    }

    /// The standable patches in one scope, in detection order.
    pub fn floors(&self, scope: Handle<Node>) -> &[FloorSurface] {
        self.floors.get(&scope).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// How many scopes have somewhere to stand.
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// No scope has any floor.
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Collider;

    fn oid(n: &str) -> ObjectId {
        ObjectId::derived("actor", n)
    }

    fn platform(name: &str, min: Vec3, max: Vec3) -> Collider {
        Collider::new(oid(name), Aabb::new(min, max))
    }

    #[test]
    fn a_box_offers_exactly_one_standable_face() {
        let mut g = CoarseGeometry::new();
        g.add(platform("slab", Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)));

        let floors = detect_floors(&g, 50.0);
        assert_eq!(floors.len(), 1, "only the top of a box is standable");
        assert_eq!(floors[0].patch.min.y, 1.0, "the patch sits on the top face");
        assert_eq!(floors[0].patch.max.y, 1.0, "and has no thickness");
    }

    #[test]
    fn the_slope_limit_is_the_only_thing_that_selects_a_floor() {
        let mut g = CoarseGeometry::new();
        g.add(platform("slab", Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)));

        // ⚠ A surface never vetoes detection — only geometry decides. At 0° the top still qualifies,
        // because it is exactly level.
        assert_eq!(detect_floors(&g, 0.0).len(), 1);
        // And no slope limit can conjure a wall into a floor.
        assert_eq!(detect_floors(&g, 89.0).len(), 1);
    }

    #[test]
    fn standing_volume_rises_from_the_patch() {
        let mut g = CoarseGeometry::new();
        g.add(platform("slab", Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)));
        let v = detect_floors(&g, 50.0)[0].standing_volume(1.9);
        assert_eq!(v.min.y, 1.0);
        assert_eq!(
            v.max.y, 2.9,
            "an occupant occupies the space above the floor"
        );
    }

    #[test]
    fn the_ladder_is_monotone() {
        // The property M06's tolerance model rests on: envelope ⊇ outer ⊇ inner. Checked by sampling,
        // because it must hold for points, not just for the boxes it was built from.
        let mut g = CoarseGeometry::new();
        g.add(platform("a", Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)));
        g.add(platform(
            "b",
            Vec3::new(6.0, 0.0, 0.0),
            Vec3::new(10.0, 1.0, 4.0),
        ));

        let floors = detect_floors(&g, 50.0);
        let b = ScopeBounds::from_floors(&floors, 2.0);

        let mut sampled = 0;
        for xi in 0..24 {
            for yi in 0..12 {
                for zi in 0..12 {
                    let p = Vec3::new(xi as f64 * 0.5, yi as f64 * 0.5, zi as f64 * 0.5);
                    if b.inner_contains(p) {
                        sampled += 1;
                        assert!(b.outer_contains(p), "inner ⊆ outer violated at {p:?}");
                        assert!(
                            b.envelope().contains_point(p),
                            "outer ⊆ envelope violated at {p:?}"
                        );
                    }
                }
            }
        }
        assert!(sampled > 0, "the sample grid never hit the inner bound");
    }

    #[test]
    fn the_gap_between_the_bounds_is_optimistic() {
        // ⚠ The softlock direction. Two platforms with a gap: the hull spans it, the inner bound does
        // not. A sightline across the gap reads *clear* at L2 and *blocked* at L4 — the direction that
        // gets re-checked rather than the one that strands a player.
        let mut g = CoarseGeometry::new();
        g.add(platform("a", Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)));
        g.add(platform(
            "b",
            Vec3::new(8.0, 0.0, 0.0),
            Vec3::new(12.0, 1.0, 4.0),
        ));

        let b = ScopeBounds::from_floors(&detect_floors(&g, 50.0), 2.0);
        let over_the_gap = Vec3::new(6.0, 1.5, 2.0);

        assert!(
            !b.inner_contains(over_the_gap),
            "there is nothing to stand on mid-gap"
        );
        assert!(
            b.outer_contains(over_the_gap),
            "the hull spans the gap — optimistic, which is the safe direction"
        );
    }

    #[test]
    fn a_point_outside_the_hull_is_a_definite_no() {
        let mut g = CoarseGeometry::new();
        g.add(platform("a", Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)));
        let b = ScopeBounds::from_floors(&detect_floors(&g, 50.0), 2.0);
        assert!(!b.outer_contains(Vec3::new(50.0, 0.5, 0.0)));
        assert!(!b.outer_contains(Vec3::new(0.0, -20.0, 0.0)));
    }

    #[test]
    fn a_scope_with_nowhere_to_stand_answers_rather_than_panicking() {
        let b = ScopeBounds::from_floors(&[], 2.0);
        assert!(b.is_empty());
        assert!(!b.inner_contains(Vec3::ZERO));
        assert!(
            !b.outer_contains(Vec3::ZERO),
            "an empty hull contains nothing"
        );
    }

    #[test]
    fn the_hull_is_a_hull_and_not_a_box() {
        // A tetrahedron: its hull must exclude the corner an AABB would include, or "outer" would be
        // no tighter than "envelope" and L2c would earn nothing over L2b.
        let pts = [
            Vec3::ZERO,
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
            Vec3::new(0.0, 0.0, 4.0),
        ];
        let h = ConvexHull::of(&pts);
        assert_eq!(h.plane_count(), 4);
        assert!(h.contains(Vec3::new(0.5, 0.5, 0.5)));
        assert!(
            !h.contains(Vec3::new(3.9, 3.9, 3.9)),
            "the far AABB corner is outside the tetrahedron"
        );
    }

    #[test]
    fn coplanar_points_give_an_empty_hull_rather_than_a_wrong_one() {
        let flat = [
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
        ];
        let h = ConvexHull::of(&flat);
        assert_eq!(h.plane_count(), 0);
        assert!(!h.contains(Vec3::new(0.5, 0.0, 0.5)));
    }

    #[test]
    fn detection_order_is_deterministic() {
        let mut g = CoarseGeometry::new();
        for i in 0..8 {
            g.add(platform(
                &format!("p{i}"),
                Vec3::new(i as f64 * 5.0, 0.0, 0.0),
                Vec3::new(i as f64 * 5.0 + 4.0, 1.0, 4.0),
            ));
        }
        let a = detect_floors(&g, 50.0);
        let b = detect_floors(&g, 50.0);
        assert_eq!(a, b);
        assert_eq!(a.len(), 8);
    }
    // --- the per-scope pass -----------------------------------------------------------------

    fn two_room_world() -> (crate::node::NodeGraph, Vec<Handle<Node>>) {
        use crate::node::{NodeGraph, NodeState};
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "reach").unwrap();
        let area = g.add_child(reach, "area").unwrap();
        let rooms: Vec<_> = (0..2)
            .map(|i| g.add_child(area, format!("room_{i}")).unwrap())
            .collect();
        for h in g.walk() {
            g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(40.0)))
                .unwrap();
            g.advance(h, NodeState::Realized).unwrap();
        }
        (g, rooms)
    }

    #[test]
    fn the_ladder_answers_per_scope() {
        let (_, rooms) = two_room_world();
        let mut geo = CoarseGeometry::new();
        geo.add(platform("a", Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)).in_scope(rooms[0]));
        geo.add(
            platform("b", Vec3::new(20.0, 0.0, 0.0), Vec3::new(24.0, 1.0, 4.0)).in_scope(rooms[1]),
        );

        let ladder = FloorLadder::build(&geo, 50.0, 2.0);
        assert_eq!(ladder.len(), 2);

        // Each room's bounds cover its own floor and not its neighbour's.
        let a = ladder.bounds(rooms[0]).expect("room 0 has floor");
        assert!(a.inner_contains(Vec3::new(2.0, 1.5, 2.0)));
        assert!(
            !a.outer_contains(Vec3::new(22.0, 1.5, 2.0)),
            "a scope answers for itself"
        );
    }

    #[test]
    fn geometry_the_graph_does_not_own_contributes_to_no_scope() {
        let mut geo = CoarseGeometry::new();
        geo.add(platform("loose", Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)));
        assert!(
            FloorLadder::build(&geo, 50.0, 2.0).is_empty(),
            "a collider with no scope has no scope to answer for"
        );
    }

    #[test]
    fn lava_is_a_floor_and_stays_in_the_graph() {
        // ⚠ P2 — gate a region, never delete it. Floor detection is geometric, so a hazard surface
        // produces floor exactly like stone does. If a surface could veto detection, hover boots could
        // never route across lava: there would be nothing there to route *through*.
        let (_, rooms) = two_room_world();
        let mut geo = CoarseGeometry::new();
        geo.add(platform("stone", Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)).in_scope(rooms[0]));
        geo.add(
            platform("lava", Vec3::new(4.0, 0.0, 0.0), Vec3::new(8.0, 1.0, 4.0)).in_scope(rooms[0]),
        );

        let ladder = FloorLadder::build(&geo, 50.0, 2.0);
        assert_eq!(ladder.floors(rooms[0]).len(), 2, "lava is floor too");
        assert!(
            ladder
                .bounds(rooms[0])
                .expect("floor")
                .inner_contains(Vec3::new(6.0, 1.5, 2.0)),
            "the lava tile is somewhere an occupant could be — whether they SHOULD is supports()"
        );
    }

    #[test]
    fn the_pass_is_reproducible() {
        let (_, rooms) = two_room_world();
        let mut geo = CoarseGeometry::new();
        for i in 0..6 {
            geo.add(
                platform(
                    &format!("p{i}"),
                    Vec3::new(i as f64 * 5.0, 0.0, 0.0),
                    Vec3::new(i as f64 * 5.0 + 4.0, 1.0, 4.0),
                )
                .in_scope(rooms[i % 2]),
            );
        }
        assert_eq!(
            FloorLadder::build(&geo, 50.0, 2.0),
            FloorLadder::build(&geo, 50.0, 2.0)
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// Elevation bands
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// One elevation band — a physical storey, across every room in the scope.
///
/// ⚠ **Banded by elevation, not by per-room floor index.** Rooms have different floor counts, so a
/// global *"floor 2"* would mean a different physical height in every room — and a layer-isolation
/// control that means something different per room is a control nobody can reason with.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    /// Its index from the bottom, `0` upward.
    pub index: usize,
    /// The lowest surface in it.
    pub low: f64,
    /// The highest.
    pub high: f64,
    /// Which surfaces fell in it, by owner.
    pub members: Vec<ObjectId>,
}

impl Band {
    /// Does an elevation fall in this band?
    pub fn contains(&self, elevation: f64) -> bool {
        elevation >= self.low && elevation <= self.high
    }

    /// How tall the band is.
    pub fn span(&self) -> f64 {
        self.high - self.low
    }
}

/// Group floor surfaces into elevation bands.
///
/// ⚠ **The tolerance is `standing_height`, and that is not a magic number.** Two surfaces a player
/// cannot stand between are the same physical level with a step in it; two they can stand between are
/// different storeys. It is a quantity the project already declares, so it scales with `world_scale`
/// and needs no dial of its own.
///
/// ▶ **Too tight and every room gets its own band; too loose and a mezzanine merges with a ground
/// floor.** Anchoring to the one length the player's body already defines is what stops the parameter
/// being a taste.
///
/// # Chaining is the failure mode, and it is bounded
///
/// ⚠ **Gap-based clustering chains.** Surfaces at `0, 1.8, 3.6, 5.4` are each within a tolerance of
/// the last, so a naive pass merges three storeys into one band — the exact mistake that makes a
/// layer-isolation control useless in the buildings it matters most for.
///
/// ▶ **So a band also closes when its span would exceed twice the tolerance.** A band is at most two
/// standing heights tall; beyond that it is not one level however smooth the gradient into it was.
pub fn bands(surfaces: &[FloorSurface], standing_height: f64) -> Vec<Band> {
    let tolerance = standing_height.max(f64::EPSILON);
    let mut sorted: Vec<(f64, ObjectId)> =
        surfaces.iter().map(|f| (f.patch.min.y, f.owner)).collect();
    // ⚠ **Sorted deterministically, by elevation then owner.** Two surfaces at the same height must
    // land in the same order on every machine, or the band a room belongs to depends on a hash.
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut out: Vec<Band> = Vec::new();
    for (elevation, owner) in sorted {
        match out.last_mut() {
            Some(band)
                if elevation - band.high <= tolerance
                    && elevation - band.low <= tolerance * 2.0 =>
            {
                band.high = elevation;
                if !band.members.contains(&owner) {
                    band.members.push(owner);
                }
            }
            _ => out.push(Band {
                index: out.len(),
                low: elevation,
                high: elevation,
                members: vec![owner],
            }),
        }
    }
    out
}

/// Which band an elevation falls in, if any.
///
/// ▶ **`None` is a real answer**, not a failure: a surface above the top band or below the bottom
/// one is off-band, and the view *dims* it rather than hiding it, so spatial context survives.
pub fn band_of(bands: &[Band], elevation: f64) -> Option<usize> {
    bands
        .iter()
        .find(|b| b.contains(elevation))
        .map(|b| b.index)
}
