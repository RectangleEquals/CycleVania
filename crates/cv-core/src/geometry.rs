//! **Coarse geometry and the spatial primitives mechanics reason through**.
//!
//! # Why "coarse"
//!
//! L4 builds the geometry a player actually sees. Everything before it — the solver, the skeleton, the
//! volume pass — has to reason spatially *without* that, using scope envelopes and reserved volumes.
//! So the primitives here answer questions about **boxes**, and they are deliberately honest about it:
//! a `raycast` result names the face of an AABB, not a triangle.
//!
//! That is not a placeholder. A mechanic asking *"can the laser reach the catcher from here?"* is
//! asking a question about layout, and layout is settled long before geometry is. When L4 lands
//! it refines what a [`Collider`] is; the questions and their shapes stay put.
//!
//! # The primitives are flow-agnostic, on purpose
//!
//! A ray does not know what a laser is. [`CoarseGeometry::raycast`] reports the first thing in the
//! way — it never asks whether that thing *blocks* the caller, because blocking is a property of the
//! interaction, not of the geometry. Glass stops a bullet and passes a laser; the glass knows that, the
//! ray does not.
//!
//! So flow-selective queries are two steps, and the split is the point:
//!
//! ```ignore
//! for hit in geometry.raycast_all(origin, dir, range) {
//!     // The Surface answers per attempt — `Interaction::RemoteUse` with a `Beam` target here.
//!     if surface_of(&hit).affords(&beam).is_open() {
//!         continue;           // glass passes the laser
//!     }
//!     break;                  // and stops the bullet
//! }
//! ```
//!
//! ⚠ **`affords` takes an `Interaction`, not a verb**, which is the whole reason one pane of glass can
//! block ballistics and walking while passing a laser and a sightline: transit is a `RemoteUse` subtree
//! a developer authors — `Sightline`, `Ballistic`, `Beam` — and the Surface answers per kind. An enum of
//! flow kinds would have made that set the core's to decide.
//!
//! [`raycast_all`](CoarseGeometry::raycast_all) returns hits **sorted by distance** so that loop is
//! correct by construction, and it returns a plain `Vec` rather than taking a predicate because this
//! surface crosses the binding seam — a closure would not translate, an array does.
//!
//! # Determinism
//!
//! Every primitive is built from `+ - * /` and comparisons, which IEEE-754 requires to be correctly
//! rounded and are therefore identical on every target. Two hazards get explicit handling:
//!
//! * **Axis-parallel rays** would divide by zero and then compute `0 * inf`, which is NaN. The slab
//!   test handles a parallel axis *explicitly* rather than leaning on `min`/`max` to swallow the NaN:
//!   the swallow trick only works when the origin is strictly inside the slab, and the NaN appears
//!   precisely when the origin lies **on** a slab boundary — which, in a world of axis-aligned boxes
//!   sharing walls, is the ordinary case rather than a corner one. No epsilon is involved either way.
//! * **Ties.** Two colliders at the same distance must come back in the same order everywhere, so hits
//!   sort by `(distance, collider index)` using a total order, never by float comparison alone.

use crate::node::{Node, NodeGraph, NodeKind, NodeState};
use crate::object::{Object, ObjectId};
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use crate::Handle;
use cv_determinism::{math, Aabb, Vec3};
use std::fmt;

// ---------------------------------------------------------------------------------------------
// Faces
// ---------------------------------------------------------------------------------------------

/// One of a box's six sides — the finest surface granularity coarse geometry has.
///
/// A face is what carries surface tags before L4 exists: "this wall is portal-able" is a statement
/// about a face, and stays meaningful when the face later becomes a set of triangles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Face {
    /// The −X side.
    NegX,
    /// The +X side.
    PosX,
    /// The −Y side — the floor of an axis-aligned box.
    NegY,
    /// The +Y side — the ceiling.
    PosY,
    /// The −Z side.
    NegZ,
    /// The +Z side.
    PosZ,
}

impl Face {
    /// All six, in tag order.
    pub const ALL: [Face; 6] = [
        Face::NegX,
        Face::PosX,
        Face::NegY,
        Face::PosY,
        Face::NegZ,
        Face::PosZ,
    ];

    /// The outward unit normal.
    pub fn normal(self) -> Vec3 {
        match self {
            Face::NegX => Vec3::new(-1.0, 0.0, 0.0),
            Face::PosX => Vec3::new(1.0, 0.0, 0.0),
            Face::NegY => Vec3::new(0.0, -1.0, 0.0),
            Face::PosY => Vec3::new(0.0, 1.0, 0.0),
            Face::NegZ => Vec3::new(0.0, 0.0, -1.0),
            Face::PosZ => Vec3::new(0.0, 0.0, 1.0),
        }
    }

    /// Which axis this face is perpendicular to: 0 = X, 1 = Y, 2 = Z.
    pub fn axis(self) -> usize {
        match self {
            Face::NegX | Face::PosX => 0,
            Face::NegY | Face::PosY => 1,
            Face::NegZ | Face::PosZ => 2,
        }
    }

    /// The face on an `axis` that a ray travelling with `direction_component` enters through.
    ///
    /// Entering means meeting the side that faces *back* along the ray, which is why the sign is
    /// inverted here — a ray heading +X enters through the −X face.
    fn entered(axis: usize, direction_component: f64) -> Face {
        match (axis, direction_component >= 0.0) {
            (0, true) => Face::NegX,
            (0, false) => Face::PosX,
            (1, true) => Face::NegY,
            (1, false) => Face::PosY,
            (2, true) => Face::NegZ,
            _ => Face::PosZ,
        }
    }

    fn tag(self) -> u8 {
        match self {
            Face::NegX => 0,
            Face::PosX => 1,
            Face::NegY => 2,
            Face::PosY => 3,
            Face::NegZ => 4,
            Face::PosZ => 5,
        }
    }

    fn from_tag(tag: u8) -> Option<Face> {
        Face::ALL.get(tag as usize).copied()
    }
}

impl fmt::Display for Face {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Face::NegX => "-x",
            Face::PosX => "+x",
            Face::NegY => "-y",
            Face::PosY => "+y",
            Face::NegZ => "-z",
            Face::PosZ => "+z",
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Colliders
// ---------------------------------------------------------------------------------------------

/// An index into a [`CoarseGeometry`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColliderId(pub u32);

impl fmt::Display for ColliderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "collider[{}]", self.0)
    }
}

/// One box in the coarse world, and the surface tags its sides carry.
#[derive(Clone, Debug, PartialEq)]
pub struct Collider {
    /// Where it is.
    pub bounds: Aabb,
    /// What it belongs to — a scope, a placed instance, an actor.
    pub owner: ObjectId,
    /// The scope it sits in, when it is not itself a scope.
    pub scope: Option<Handle<Node>>,
    /// Tags carried by every face.
    tags: Vec<ObjectId>,
    /// Tags carried by one face only, sorted so the collider is canonical.
    face_tags: Vec<(Face, ObjectId)>,
}

impl Collider {
    /// A bare collider with no tags.
    pub fn new(owner: ObjectId, bounds: Aabb) -> Self {
        Collider {
            bounds,
            owner,
            scope: None,
            tags: Vec::new(),
            face_tags: Vec::new(),
        }
    }

    /// Record which scope this sits in.
    pub fn in_scope(mut self, scope: Handle<Node>) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Tag every face.
    pub fn tagged(mut self, tag: ObjectId) -> Self {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
            self.tags.sort_unstable();
        }
        self
    }

    /// Tag one face — "the north wall is portal-able, the rest are not".
    pub fn tagged_face(mut self, face: Face, tag: ObjectId) -> Self {
        let entry = (face, tag);
        if !self.face_tags.contains(&entry) {
            self.face_tags.push(entry);
            self.face_tags.sort_unstable();
        }
        self
    }

    /// Every tag on a face: the collider-wide ones plus that face's own, in id order.
    pub fn tags_on(&self, face: Face) -> Vec<ObjectId> {
        let mut out = self.tags.clone();
        out.extend(
            self.face_tags
                .iter()
                .filter(|(f, _)| *f == face)
                .map(|(_, t)| *t),
        );
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Does this face carry a tag?
    pub fn has_tag(&self, face: Face, tag: ObjectId) -> bool {
        self.tags.contains(&tag) || self.face_tags.contains(&(face, tag))
    }

    /// Tags that apply to every face.
    pub fn tags(&self) -> &[ObjectId] {
        &self.tags
    }
}

// ---------------------------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------------------------

/// Where a ray or sweep met something.
///
/// Deliberately plain data — `Copy`, no borrows, no tag lists — so it can be returned by value,
/// stored, compared, and eventually handed across the script boundary without lifetimes coming with
/// it. Ask the geometry for tags (`geometry.tags_at(&hit)`) rather than carrying them here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Where the ray met the surface.
    pub point: Vec3,
    /// The surface's outward normal.
    pub normal: Vec3,
    /// How far along the ray, in world units.
    pub distance: f64,
    /// Which collider.
    pub collider: ColliderId,
    /// What it belongs to — usually what a mechanic actually cares about.
    pub owner: ObjectId,
    /// Which side was met.
    pub face: Face,
    /// The ray started inside this collider, so `distance` is 0 and the surface was met from within.
    ///
    /// Worth distinguishing: an emitter inside its own bounds is a normal situation, and a caller that
    /// silently treated it as an obstruction would produce a laser that blocks itself.
    pub from_inside: bool,
}

/// The outcome of moving a box until it touches something.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sweep {
    /// How far the box actually got.
    pub distance: f64,
    /// Where its centre ended up.
    pub end: Vec3,
    /// What stopped it, if anything.
    pub hit: Option<Hit>,
}

impl Sweep {
    /// Did it travel the whole way unobstructed?
    pub fn is_clear(&self) -> bool {
        self.hit.is_none()
    }
}

// ---------------------------------------------------------------------------------------------
// The coarse world
// ---------------------------------------------------------------------------------------------

/// The boxes the spatial primitives run against.
///
/// Populated from scope envelopes before L4 exists, and from reserved volumes once it does. A linear
/// scan is deliberate at this size — a Reach holds tens of boxes, and a broadphase structure would be
/// a source of ordering nondeterminism bought with no measurable time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoarseGeometry {
    colliders: Vec<Collider>,
}

impl CoarseGeometry {
    /// An empty world.
    pub fn new() -> Self {
        CoarseGeometry::default()
    }

    /// Every **realized** scope of a kind, as one box each.
    ///
    /// Realized only: a projected scope's envelope is a forecast, and a mechanic that raycast against
    /// forecasts would get answers that change under backtracking — the same coherence rule the
    /// counting queries follow.
    pub fn from_scopes(graph: &NodeGraph, kind: NodeKind) -> Self {
        let mut geometry = CoarseGeometry::new();
        for h in graph.walk() {
            let Some(node) = graph.get(h) else { continue };
            if node.kind() != kind || node.state() != NodeState::Realized {
                continue;
            }
            if let Some(bounds) = node.envelope() {
                geometry.add(Collider::new(node.id(), bounds).in_scope(h));
            }
        }
        geometry
    }

    /// Add a collider, returning its id.
    pub fn add(&mut self, collider: Collider) -> ColliderId {
        let id = ColliderId(self.colliders.len() as u32);
        self.colliders.push(collider);
        id
    }

    /// How many colliders.
    pub fn len(&self) -> usize {
        self.colliders.len()
    }

    /// Is the world empty?
    pub fn is_empty(&self) -> bool {
        self.colliders.is_empty()
    }

    /// Look one up.
    pub fn get(&self, id: ColliderId) -> Option<&Collider> {
        self.colliders.get(id.0 as usize)
    }

    /// Every collider, in insertion order.
    pub fn colliders(&self) -> &[Collider] {
        &self.colliders
    }

    /// The surface tags at a hit — the collider's own plus that face's.
    pub fn tags_at(&self, hit: &Hit) -> Vec<ObjectId> {
        self.get(hit.collider)
            .map(|c| c.tags_on(hit.face))
            .unwrap_or_default()
    }

    /// Does the surface a hit landed on carry a tag?
    pub fn has_tag_at(&self, hit: &Hit, tag: ObjectId) -> bool {
        self.get(hit.collider)
            .is_some_and(|c| c.has_tag(hit.face, tag))
    }

    // --- overlap ------------------------------------------------------------------------------

    /// Every collider intersecting a box, in insertion order.
    pub fn overlap(&self, bounds: Aabb) -> Vec<ColliderId> {
        self.colliders
            .iter()
            .enumerate()
            .filter(|(_, c)| c.bounds.intersects(&bounds))
            .map(|(i, _)| ColliderId(i as u32))
            .collect()
    }

    /// Every collider containing a point.
    pub fn overlap_point(&self, p: Vec3) -> Vec<ColliderId> {
        self.colliders
            .iter()
            .enumerate()
            .filter(|(_, c)| c.bounds.contains_point(p))
            .map(|(i, _)| ColliderId(i as u32))
            .collect()
    }

    // --- raycast ------------------------------------------------------------------------------

    /// The first thing in the way, or `None` if the ray runs clear.
    ///
    /// "First" is geometric, not semantic — see the module docs on why this does not take a
    /// the attempt, via [`Surface::affords`](crate::Surface::affords).
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f64) -> Option<Hit> {
        self.raycast_all(origin, direction, max_distance)
            .into_iter()
            .next()
    }

    /// Everything the ray meets, **nearest first**.
    ///
    /// The building block for flow-selective queries: walk the list and stop at the first surface that
    /// blocks whatever is travelling. Ties break on collider index, so two coincident boxes always come
    /// back in the same order.
    pub fn raycast_all(&self, origin: Vec3, direction: Vec3, max_distance: f64) -> Vec<Hit> {
        let Some(dir) = normalize(direction) else {
            return Vec::new();
        };
        let inv = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

        let mut hits: Vec<Hit> = Vec::new();
        for (i, collider) in self.colliders.iter().enumerate() {
            if let Some(hit) = ray_box(origin, dir, inv, max_distance, &collider.bounds) {
                hits.push(Hit {
                    collider: ColliderId(i as u32),
                    owner: collider.owner,
                    ..hit
                });
            }
        }
        // A total order: distance first, then index. Sorting on the float alone would leave coincident
        // boxes in whatever order the scan happened to produce.
        hits.sort_by(|a, b| {
            a.distance
                .total_cmp(&b.distance)
                .then(a.collider.cmp(&b.collider))
        });
        hits
    }

    /// Is the straight line between two points unobstructed?
    ///
    /// Purely geometric: *anything* blocks it. Flow-selective sight — "can the turret see through the
    /// glass?" — is [`raycast_all`](Self::raycast_all) plus the mechanic's own `blocks`.
    pub fn line_of_sight(&self, from: Vec3, to: Vec3) -> bool {
        let delta = to - from;
        let distance = delta.length();
        // Exactly zero, not a tolerance: this asks "are these the same point", which is a degenerate
        // case with no ray to trace, rather than "are these two lengths equal". The binding
        // contract's no-float-equality rule is about the latter, and widening this to an epsilon
        // would silently answer `true` for short-but-real sightlines.
        if distance == 0.0 {
            return true;
        }
        !self
            .raycast_all(from, delta, distance)
            .iter()
            .any(|h| !h.from_inside)
    }

    // --- sweep and slide ----------------------------------------------------------------------

    /// Move a box along a direction until it touches something.
    ///
    /// Implemented by **Minkowski expansion**: growing each obstacle by the moving box's half-extents
    /// turns "where does this box first touch?" into "where does this *ray* first hit?", which is the
    /// problem already solved above. Exact, and it reuses one piece of arithmetic rather than adding a
    /// second one to keep bit-identical.
    pub fn sweep(&self, box_: Aabb, direction: Vec3, max_distance: f64) -> Sweep {
        let centre = box_.center();
        let half = box_.half_extents();
        let Some(dir) = normalize(direction) else {
            return Sweep {
                distance: 0.0,
                end: centre,
                hit: None,
            };
        };
        let inv = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

        let mut best: Option<Hit> = None;
        for (i, collider) in self.colliders.iter().enumerate() {
            let expanded = Aabb::new(collider.bounds.min - half, collider.bounds.max + half);
            let Some(hit) = ray_box(centre, dir, inv, max_distance, &expanded) else {
                continue;
            };
            // A box already overlapping at the start is not something to collide *with* — it is
            // something to move out of. Reporting it as a stop at distance 0 would freeze anything that
            // began the frame intersecting, which is the classic sweep bug.
            if hit.from_inside {
                continue;
            }
            let hit = Hit {
                collider: ColliderId(i as u32),
                owner: collider.owner,
                ..hit
            };
            let better = match &best {
                None => true,
                Some(b) => hit
                    .distance
                    .total_cmp(&b.distance)
                    .then(hit.collider.cmp(&b.collider))
                    .is_lt(),
            };
            if better {
                best = Some(hit);
            }
        }

        let distance = best.map(|h| h.distance).unwrap_or(max_distance);
        Sweep {
            distance,
            end: centre + dir * distance,
            hit: best,
        }
    }

    /// Sweep, then slide along whatever was hit and sweep again.
    ///
    /// The movement primitive a walking mechanic wants: hitting a wall at a glancing angle should carry
    /// you along it, not stop you dead. One slide iteration, deliberately — chaining them approaches a
    /// physics solver, and coarse boxes do not deserve one.
    pub fn slide_to_collision(&self, box_: Aabb, direction: Vec3, max_distance: f64) -> Sweep {
        let first = self.sweep(box_, direction, max_distance);
        let Some(hit) = first.hit else { return first };
        let Some(dir) = normalize(direction) else {
            return first;
        };

        let remaining = max_distance - first.distance;
        if remaining <= 0.0 {
            return first;
        }
        // What is left of the motion, projected onto the surface plane.
        let slide = (dir * remaining).reject_from(hit.normal);
        let Some(slide_dir) = normalize(slide) else {
            return first;
        };

        let half = box_.half_extents();
        // Step off the surface by the contact normal so the second sweep does not start touching.
        let start = Aabb::from_center_extents(first.end + hit.normal * CONTACT_SKIN, half);
        let second = self.sweep(start, slide_dir, slide.length());
        Sweep {
            distance: first.distance + second.distance,
            end: second.end,
            hit: second.hit.or(Some(hit)),
        }
    }

    /// Reflect a direction off a surface normal — the mirror rule, exposed here so a mechanic reaches
    /// for one geometry surface rather than two.
    pub fn reflect(incoming: Vec3, normal: Vec3) -> Vec3 {
        incoming.reflect(normal)
    }
}

/// How far `slide_to_collision` steps off a contact before continuing.
///
/// A fixed world-unit constant rather than a relative epsilon: relative epsilons vary with position, so
/// the same slide near the origin and far from it would behave differently — a reproducibility trap.
const CONTACT_SKIN: f64 = 1e-6;

/// Normalize, or `None` for a zero/degenerate direction.
///
/// Returning `None` rather than a default direction means a caller that passes a zero vector gets
/// nothing back instead of a confident answer about an arbitrary axis.
fn normalize(v: Vec3) -> Option<Vec3> {
    let length_squared = v.length_squared();
    // `is_finite` first: it rejects NaN and infinity, after which the comparison is meaningful. Written
    // in this order rather than as a negated `>` so there is no reliance on NaN comparison semantics.
    if !length_squared.is_finite() || length_squared <= 0.0 {
        return None;
    }
    Some(v / v.length())
}

/// The slab test: where does a ray meet a box, and through which face?
///
/// `inv` is passed in rather than recomputed per box — it is the same for every collider on one cast,
/// and computing it once keeps the arithmetic identical across them.
fn ray_box(origin: Vec3, dir: Vec3, inv: Vec3, max_distance: f64, bounds: &Aabb) -> Option<Hit> {
    let o = origin.to_array();
    let d = dir.to_array();
    let iv = inv.to_array();
    let lo = bounds.min.to_array();
    let hi = bounds.max.to_array();

    let mut t_near = 0.0f64;
    let mut t_far = max_distance;
    let mut axis = 0usize;
    let mut entered = false;

    for i in 0..3 {
        // A ray parallel to this slab either lies within it for its whole length or never meets it —
        // there is no crossing to solve for. Handled explicitly rather than by arithmetic, because the
        // arithmetic form computes `0 * inf` whenever the origin sits *on* a slab boundary, and in a
        // world of axis-aligned boxes sharing walls that is the common case, not a corner one. Relying
        // on min/max to swallow the resulting NaN would work only when the origin is strictly inside.
        if d[i] == 0.0 {
            if o[i] < lo[i] || o[i] > hi[i] {
                return None;
            }
            continue;
        }
        let t1 = (lo[i] - o[i]) * iv[i];
        let t2 = (hi[i] - o[i]) * iv[i];
        let near = math::min(t1, t2);
        let far = math::max(t1, t2);
        if near > t_near {
            t_near = near;
            axis = i;
            entered = true;
        }
        t_far = math::min(t_far, far);
        if t_far < t_near {
            return None;
        }
    }

    // Never entered a slab from outside ⇒ the origin was already within every one of them.
    let from_inside = !entered;
    if from_inside && !bounds.contains_point(origin) {
        return None;
    }
    let face = Face::entered(axis, d[axis]);
    Some(Hit {
        point: origin + dir * t_near,
        normal: if from_inside { -dir } else { face.normal() },
        distance: t_near,
        collider: ColliderId(0), // filled in by the caller, which knows the index
        owner: ObjectId::NONE,
        face,
        from_inside,
    })
}

// ---------------------------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------------------------

impl Serialize for Face {
    fn serialize(&self, w: &mut Writer) {
        w.u8(self.tag());
    }
}

impl Deserialize for Face {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Face::from_tag(r.u8()?).ok_or(SerError::InvalidValue("unknown Face tag"))
    }
}

impl Serialize for ColliderId {
    fn serialize(&self, w: &mut Writer) {
        w.u32(self.0);
    }
}

impl Deserialize for ColliderId {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(ColliderId(r.u32()?))
    }
}

impl Serialize for Hit {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.point);
        w.write(&self.normal);
        w.f64(self.distance);
        w.write(&self.collider);
        w.write(&self.owner);
        w.write(&self.face);
        w.bool(self.from_inside);
    }
}

impl Deserialize for Hit {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Hit {
            point: r.read()?,
            normal: r.read()?,
            distance: r.f64()?,
            collider: r.read()?,
            owner: r.read()?,
            face: r.read()?,
            from_inside: r.bool()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(name: &str) -> ObjectId {
        ObjectId::derived("actor", name)
    }

    fn unit_box_at(x: f64) -> Aabb {
        Aabb::new(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 1.0, 1.0, 1.0))
    }

    /// Three unit boxes in a row along +X, at x = 2, 4 and 6.
    fn row() -> CoarseGeometry {
        let mut g = CoarseGeometry::new();
        g.add(Collider::new(oid("a"), unit_box_at(2.0)));
        g.add(Collider::new(oid("b"), unit_box_at(4.0)));
        g.add(Collider::new(oid("c"), unit_box_at(6.0)));
        g
    }

    #[test]
    fn a_ray_reports_the_nearest_hit_and_the_face_it_entered() {
        let g = row();
        let hit = g
            .raycast(Vec3::new(0.0, 0.5, 0.5), Vec3::X, 100.0)
            .expect("the row is in the way");
        assert_eq!(hit.distance, 2.0);
        assert_eq!(hit.owner, oid("a"));
        assert_eq!(
            hit.face,
            Face::NegX,
            "entered through the side facing the ray"
        );
        assert_eq!(hit.normal, Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(hit.point, Vec3::new(2.0, 0.5, 0.5));
        assert!(!hit.from_inside);
    }

    #[test]
    fn raycast_all_is_sorted_and_covers_everything_in_the_way() {
        let g = row();
        let hits = g.raycast_all(Vec3::new(0.0, 0.5, 0.5), Vec3::X, 100.0);
        assert_eq!(hits.len(), 3);
        let distances: Vec<f64> = hits.iter().map(|h| h.distance).collect();
        assert_eq!(distances, vec![2.0, 4.0, 6.0]);
        // This is what makes the flow-selective march correct: stop at the first blocker.
        let first_blocking = hits.iter().find(|h| h.owner == oid("b")).unwrap();
        assert_eq!(first_blocking.distance, 4.0);
    }

    #[test]
    fn a_ray_stops_at_its_range() {
        let g = row();
        assert!(g.raycast(Vec3::new(0.0, 0.5, 0.5), Vec3::X, 1.5).is_none());
        assert!(g.raycast(Vec3::new(0.0, 0.5, 0.5), Vec3::X, 2.0).is_some());
        assert_eq!(
            g.raycast_all(Vec3::new(0.0, 0.5, 0.5), Vec3::X, 5.0).len(),
            2,
            "range clips the far box"
        );
    }

    #[test]
    fn a_ray_that_misses_returns_nothing() {
        let g = row();
        // Above the boxes.
        assert!(g
            .raycast(Vec3::new(0.0, 5.0, 0.5), Vec3::X, 100.0)
            .is_none());
        // Pointing away.
        assert!(g
            .raycast(Vec3::new(0.0, 0.5, 0.5), -Vec3::X, 100.0)
            .is_none());
    }

    #[test]
    fn an_axis_parallel_ray_grazing_a_slab_does_not_produce_nan() {
        // The hazard the module docs call out: `inv` is infinite on two axes, and `inf * 0` is NaN.
        // A naive slab test returns garbage here rather than a clean miss or hit.
        let mut g = CoarseGeometry::new();
        g.add(Collider::new(oid("wall"), unit_box_at(2.0)));

        // Travelling exactly along the box's -Y face plane.
        let along = g.raycast(Vec3::new(0.0, 0.0, 0.5), Vec3::X, 100.0);
        assert!(
            along.is_some(),
            "a ray in the boundary plane still meets the box"
        );
        assert!(along.unwrap().distance.is_finite());

        // Exactly outside it on the same axis: a clean miss, not a NaN.
        let outside = g.raycast(Vec3::new(0.0, -0.5, 0.5), Vec3::X, 100.0);
        assert!(outside.is_none());

        // And a ray parallel to a face it can never reach.
        let parallel = g.raycast(Vec3::new(0.0, 0.5, 0.5), Vec3::Y, 100.0);
        assert!(parallel.is_none());
    }

    #[test]
    fn a_ray_starting_inside_says_so_rather_than_pretending_to_be_blocked() {
        // An emitter inside its own bounds is normal; treating it as an obstruction would make a laser
        // block itself.
        let mut g = CoarseGeometry::new();
        g.add(Collider::new(oid("room"), unit_box_at(0.0)));
        let hit = g.raycast(Vec3::new(0.5, 0.5, 0.5), Vec3::X, 10.0).unwrap();
        assert!(hit.from_inside);
        assert_eq!(hit.distance, 0.0);
    }

    #[test]
    fn ties_break_deterministically_rather_than_by_scan_order() {
        let mut g = CoarseGeometry::new();
        // Two coincident boxes: distance alone cannot order them.
        g.add(Collider::new(oid("first"), unit_box_at(2.0)));
        g.add(Collider::new(oid("second"), unit_box_at(2.0)));
        let hits = g.raycast_all(Vec3::new(0.0, 0.5, 0.5), Vec3::X, 10.0);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].distance, hits[1].distance);
        assert_eq!(hits[0].collider, ColliderId(0));
        assert_eq!(hits[1].collider, ColliderId(1));
        // Repeatable, not merely once.
        for _ in 0..8 {
            let again = g.raycast_all(Vec3::new(0.0, 0.5, 0.5), Vec3::X, 10.0);
            assert_eq!(again, hits);
        }
    }

    #[test]
    fn line_of_sight_is_geometric_and_ignores_the_box_you_stand_in() {
        let g = row();
        assert!(!g.line_of_sight(Vec3::new(0.0, 0.5, 0.5), Vec3::new(8.0, 0.5, 0.5)));
        assert!(g.line_of_sight(Vec3::new(0.0, 5.0, 0.5), Vec3::new(8.0, 5.0, 0.5)));
        // Standing inside a box does not blind you.
        let mut inside = CoarseGeometry::new();
        inside.add(Collider::new(
            oid("room"),
            Aabb::new(Vec3::ZERO, Vec3::splat(10.0)),
        ));
        assert!(inside.line_of_sight(Vec3::splat(1.0), Vec3::splat(9.0)));
        // A point sees itself.
        assert!(g.line_of_sight(Vec3::ZERO, Vec3::ZERO));
    }

    #[test]
    fn overlap_finds_boxes_by_volume_and_by_point() {
        let g = row();
        let ids = g.overlap(Aabb::new(
            Vec3::new(1.5, 0.0, 0.0),
            Vec3::new(4.5, 1.0, 1.0),
        ));
        assert_eq!(ids, vec![ColliderId(0), ColliderId(1)]);
        assert_eq!(
            g.overlap_point(Vec3::new(4.5, 0.5, 0.5)),
            vec![ColliderId(1)]
        );
        assert!(g.overlap_point(Vec3::new(100.0, 0.0, 0.0)).is_empty());
    }

    #[test]
    fn a_sweep_stops_a_box_at_contact_not_at_overlap() {
        let g = row();
        // A half-unit cube centred at x=0 moving +X: it should stop with its face on the box at x=2.
        let mover = Aabb::from_center_extents(Vec3::new(0.0, 0.5, 0.5), Vec3::splat(0.5));
        let sweep = g.sweep(mover, Vec3::X, 100.0);
        assert!(!sweep.is_clear());
        assert_eq!(sweep.distance, 1.5, "1.5 to bring its +X face to x=2");
        assert_eq!(sweep.end, Vec3::new(1.5, 0.5, 0.5));
        assert_eq!(sweep.hit.unwrap().owner, oid("a"));
    }

    #[test]
    fn a_clear_sweep_travels_the_whole_way() {
        let g = row();
        let mover = Aabb::from_center_extents(Vec3::new(0.0, 5.0, 0.5), Vec3::splat(0.5));
        let sweep = g.sweep(mover, Vec3::X, 10.0);
        assert!(sweep.is_clear());
        assert_eq!(sweep.distance, 10.0);
        assert_eq!(sweep.end, Vec3::new(10.0, 5.0, 0.5));
    }

    #[test]
    fn a_sweep_that_starts_overlapping_does_not_freeze() {
        // The classic sweep bug: reporting an already-overlapping box as a contact at distance 0 traps
        // anything that begins inside something.
        let mut g = CoarseGeometry::new();
        g.add(Collider::new(oid("floor"), unit_box_at(0.0)));
        let mover = Aabb::from_center_extents(Vec3::new(0.5, 0.5, 0.5), Vec3::splat(0.25));
        let sweep = g.sweep(mover, Vec3::X, 5.0);
        assert!(sweep.distance > 0.0, "it must be able to move out");
    }

    #[test]
    fn sliding_carries_a_glancing_hit_along_the_surface() {
        // Into a wall at 45°: stopping dead would be wrong, and the movement mechanic wants the slide.
        let mut g = CoarseGeometry::new();
        g.add(Collider::new(
            oid("wall"),
            Aabb::new(Vec3::new(2.0, -10.0, -10.0), Vec3::new(3.0, 10.0, 10.0)),
        ));
        let mover = Aabb::from_center_extents(Vec3::ZERO, Vec3::splat(0.5));
        let direction = Vec3::new(1.0, 0.0, 1.0);

        let straight = g.sweep(mover, direction, 10.0);
        let slid = g.slide_to_collision(mover, direction, 10.0);
        assert!(
            slid.distance > straight.distance,
            "sliding must make progress"
        );
        // It ends up alongside the wall, not through it.
        assert!(
            slid.end.x <= 1.5 + 1e-6,
            "did not tunnel: x = {}",
            slid.end.x
        );
        assert!(
            slid.end.z > straight.end.z,
            "and it travelled along the face"
        );
    }

    #[test]
    fn reflect_is_the_mirror_rule() {
        let d = Vec3::new(1.0, -1.0, 0.0).normalized();
        let r = CoarseGeometry::reflect(d, Vec3::Y);
        assert!((r - Vec3::new(1.0, 1.0, 0.0).normalized()).length() < 1e-12);
    }

    #[test]
    fn tags_combine_collider_wide_and_per_face() {
        let portalable = ObjectId::derived("surface", "portalable");
        let structural = ObjectId::derived("surface", "structural");
        let mut g = CoarseGeometry::new();
        let id = g.add(
            Collider::new(oid("room"), unit_box_at(2.0))
                .tagged(structural)
                .tagged_face(Face::NegX, portalable),
        );
        let c = g.get(id).unwrap();
        assert_eq!(
            c.tags_on(Face::NegX),
            vec![portalable.min(structural), portalable.max(structural)]
        );
        assert_eq!(c.tags_on(Face::PosX), vec![structural]);
        assert!(c.has_tag(Face::NegX, portalable));
        assert!(!c.has_tag(Face::PosX, portalable));

        // And a hit reads them back through the geometry rather than carrying them.
        let hit = g.raycast(Vec3::new(0.0, 0.5, 0.5), Vec3::X, 10.0).unwrap();
        assert!(
            g.has_tag_at(&hit, portalable),
            "the -x face was the one hit"
        );
        assert!(g.tags_at(&hit).contains(&structural));
    }

    #[test]
    fn geometry_from_scopes_takes_realized_envelopes_only() {
        use crate::node::NodeState;
        let mut graph = NodeGraph::new(1.0, 1);
        let reach = graph.add_child(graph.root(), "reach").unwrap();
        let area = graph.add_child(reach, "area").unwrap();
        let a = graph.add_child(area, "space_a").unwrap();
        let b = graph.add_child(area, "space_b").unwrap();
        for h in [graph.root(), reach, area, a, b] {
            graph
                .set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(4.0)))
                .unwrap();
        }
        for h in [graph.root(), reach, area, a] {
            graph.advance(h, NodeState::Realized).unwrap();
        }
        // `b` is still a forecast.
        let g = CoarseGeometry::from_scopes(&graph, NodeKind::Space);
        assert_eq!(
            g.len(),
            1,
            "a projected envelope is a forecast, not geometry"
        );
        assert_eq!(g.colliders()[0].scope, Some(a));
    }

    #[test]
    fn a_degenerate_direction_answers_nothing_rather_than_guessing() {
        let g = row();
        assert!(g.raycast(Vec3::ZERO, Vec3::ZERO, 10.0).is_none());
        assert!(g.raycast_all(Vec3::ZERO, Vec3::ZERO, 10.0).is_empty());
        let mover = Aabb::from_center_extents(Vec3::ZERO, Vec3::splat(0.5));
        let sweep = g.sweep(mover, Vec3::ZERO, 10.0);
        assert_eq!(sweep.distance, 0.0);
        assert!(sweep.is_clear());
    }

    #[test]
    fn results_round_trip() {
        use crate::serialize::{from_bytes, to_bytes};
        let g = row();
        let hit = g.raycast(Vec3::new(0.0, 0.5, 0.5), Vec3::X, 10.0).unwrap();
        let bytes = to_bytes(&hit);
        assert_eq!(from_bytes::<Hit>(&bytes).unwrap(), hit);
        for face in Face::ALL {
            assert_eq!(from_bytes::<Face>(&to_bytes(&face)).unwrap(), face);
        }
    }
}
