//! **The three-axis query builder** — what to trace × what to consider × what to report.
//!
//! ```text
//! ctx.query()
//!    .ray(origin, direction, range)        ── AXIS 1: what to trace
//!    .exclude_self()                       ── AXIS 2: what to consider
//!    .only_kind(k)
//!    .with_tag(q)
//!    .detail(Detail::Collider)             ── AXIS 3: what to report
//!    .min_fidelity(Fidelity::Hull)
//!    .all()                                ── terminal: all / first / nearest / any / count
//! ```
//!
//! Three axes rather than a family of functions, because they are genuinely independent: *what* you
//! trace has nothing to do with *which* things count or *how much* you want back. A per-combination
//! function set would be the product of the three, and most of it would never be called.
//!
//! # Declarative filters, never closures
//!
//! ⚠ **A predicate callback would survive neither the binding contract nor the visual palette.** Id
//! lists, kind masks and tag queries are data: they cross to TypeScript, they render as pins a
//! developer picks from, and the VM can evaluate them without calling back into content. A closure
//! does none of that — and the moment one is accepted here, the same query stops being expressible in
//! the editor at all.
//!
//! # `only_realized()` is the coherence guard, and it is on by default
//!
//! ⚠ Forecast content — things the pipeline expects to place but has not — is **excluded unless a
//! caller explicitly asks for it**. Default-on is the whole point: a hook that forgot to think about
//! forecasts cannot accidentally reason about content that may never exist, and the failure mode of
//! forgetting is a *narrower* answer rather than a wrong one.
//!
//! # Detail is what you ask for; fidelity is what exists
//!
//! Asking for `Triangle` detail at `Envelope` fidelity is not an error and does not return triangles —
//! it returns what exists, labelled with what it is. ⚠ **Fields below the achieved detail are absent,
//! which is checkable**, rather than present-and-meaningless.

use crate::geometry::{CoarseGeometry, Hit};
use crate::node::Node;
use crate::object::ObjectId;
use crate::trivalent::Fidelity;
use crate::Handle;
use cv_determinism::{Aabb, Vec3};

/// How much a query wants back.
///
/// ⚠ **Detail is what you ASK for; fidelity is what EXISTS.** Conflating them is how a caller ends up
/// believing it received triangles from a world that has only boxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Detail {
    /// Which room. Available from L1.
    Scope,
    /// Which box, which face. From L2.
    Collider,
    /// Which placed actor, with its content path. From L2.
    Instance,
    /// Which polygon. From L3.
    Polygon,
    /// Which triangle, with an interpolated normal. From L4.
    Triangle,
}

impl Detail {
    /// The finest detail this fidelity can actually supply.
    ///
    /// ⚠ Asking for more is legal and returns what exists — the answer is *labelled*, not padded.
    pub fn available_at(fidelity: Fidelity) -> Detail {
        match fidelity {
            Fidelity::Envelope => Detail::Scope,
            Fidelity::Hull => Detail::Instance,
            Fidelity::Geometry => Detail::Triangle,
        }
    }

    /// Is this detail obtainable at that fidelity?
    pub fn is_available_at(self, fidelity: Fidelity) -> bool {
        self <= Detail::available_at(fidelity)
    }
}

/// **Axis 1** — what to trace.
#[derive(Clone, Debug, PartialEq)]
pub enum Trace {
    /// A ray from a point.
    Ray {
        origin: Vec3,
        direction: Vec3,
        range: f64,
    },
    /// A finite segment — line of sight between two known points.
    Segment { from: Vec3, to: Vec3 },
    /// Everything within a radius.
    Sphere { centre: Vec3, radius: f64 },
    /// Everything already inside a volume — the placement-validity workhorse.
    Overlap { volume: Aabb },
    /// A swept box — *"does this fit if I move it here?"*
    BoxCast {
        volume: Aabb,
        direction: Vec3,
        range: f64,
    },
    /// A directional detection cone.
    Cone {
        origin: Vec3,
        axis: Vec3,
        angle: f64,
        range: f64,
    },
}

/// **Axis 2** — what to consider. Declarative, and therefore translatable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Consider {
    /// Only these ids, if non-empty.
    pub only: Vec<ObjectId>,
    /// Never these ids.
    pub exclude: Vec<ObjectId>,
    /// Only content of these kinds, if non-empty.
    pub only_kinds: Vec<ObjectId>,
    /// Only inside this scope.
    pub only_scope: Option<Handle<Node>>,
    /// Must carry all of these tags.
    pub with_tags: Vec<ObjectId>,
    /// Must carry none of these tags.
    pub without_tags: Vec<ObjectId>,
    /// ⚠ **The coherence guard.** Defaults to `true` via [`Query::new`].
    pub only_realized: bool,
}

impl Consider {
    /// Does a candidate survive every filter?
    ///
    /// ⚠ Order is irrelevant to the result but not to cost: cheap identity tests run before tag
    /// lookups, because a query over a large scope evaluates this for every candidate.
    pub fn admits(&self, id: ObjectId, kind: Option<ObjectId>, tags: &[ObjectId]) -> bool {
        if self.exclude.contains(&id) {
            return false;
        }
        if !self.only.is_empty() && !self.only.contains(&id) {
            return false;
        }
        if !self.only_kinds.is_empty() && !kind.is_some_and(|k| self.only_kinds.contains(&k)) {
            return false;
        }
        if !self.with_tags.iter().all(|t| tags.contains(t)) {
            return false;
        }
        if self.without_tags.iter().any(|t| tags.contains(t)) {
            return false;
        }
        true
    }
}

/// A spatial query under construction.
///
/// ⚠ **Built and then run**, rather than executed as it is described, so the whole shape is available
/// to the VM and the editor as data before anything is traced.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    trace: Option<Trace>,
    consider: Consider,
    detail: Detail,
    min_fidelity: Fidelity,
}

impl Default for Query {
    fn default() -> Self {
        Query::new()
    }
}

impl Query {
    /// An empty query, with the coherence guard **on**.
    pub fn new() -> Self {
        Query {
            trace: None,
            consider: Consider {
                only_realized: true,
                ..Default::default()
            },
            detail: Detail::Collider,
            min_fidelity: Fidelity::Envelope,
        }
    }

    // --- Axis 1: what to trace ------------------------------------------------------------------

    /// Trace a ray.
    pub fn ray(mut self, origin: Vec3, direction: Vec3, range: f64) -> Self {
        self.trace = Some(Trace::Ray {
            origin,
            direction,
            range,
        });
        self
    }

    /// Trace between two known points.
    pub fn segment(mut self, from: Vec3, to: Vec3) -> Self {
        self.trace = Some(Trace::Segment { from, to });
        self
    }

    /// Everything within a radius.
    pub fn sphere(mut self, centre: Vec3, radius: f64) -> Self {
        self.trace = Some(Trace::Sphere { centre, radius });
        self
    }

    /// What is already inside a volume.
    pub fn overlap(mut self, volume: Aabb) -> Self {
        self.trace = Some(Trace::Overlap { volume });
        self
    }

    /// Sweep a box along a direction.
    pub fn box_cast(mut self, volume: Aabb, direction: Vec3, range: f64) -> Self {
        self.trace = Some(Trace::BoxCast {
            volume,
            direction,
            range,
        });
        self
    }

    /// A detection cone.
    pub fn cone(mut self, origin: Vec3, axis: Vec3, angle: f64, range: f64) -> Self {
        self.trace = Some(Trace::Cone {
            origin,
            axis,
            angle,
            range,
        });
        self
    }

    // --- Axis 2: what to consider ---------------------------------------------------------------

    /// Ignore these.
    pub fn exclude(mut self, ids: impl IntoIterator<Item = ObjectId>) -> Self {
        self.consider.exclude.extend(ids);
        self
    }

    /// Consider only these.
    pub fn only(mut self, ids: impl IntoIterator<Item = ObjectId>) -> Self {
        self.consider.only.extend(ids);
        self
    }

    /// Ignore the caller itself — the commonest exclusion by far.
    pub fn exclude_self(self, own: ObjectId) -> Self {
        self.exclude([own])
    }

    /// Only content of this kind.
    pub fn only_kind(mut self, kind: ObjectId) -> Self {
        self.consider.only_kinds.push(kind);
        self
    }

    /// Only inside this scope.
    pub fn only_scope(mut self, scope: Handle<Node>) -> Self {
        self.consider.only_scope = Some(scope);
        self
    }

    /// Must carry this tag.
    pub fn with_tag(mut self, tag: ObjectId) -> Self {
        self.consider.with_tags.push(tag);
        self
    }

    /// Must not carry this tag.
    pub fn without_tag(mut self, tag: ObjectId) -> Self {
        self.consider.without_tags.push(tag);
        self
    }

    /// ⚠ **Opt back in to forecast content.**
    ///
    /// The guard is on by default, so this is the only way to see things the pipeline expects to
    /// place but has not. Naming it explicitly is the point: a hook that reasons about forecasts has
    /// said so, and a reader can find every such hook by searching for this one call.
    pub fn including_forecasts(mut self) -> Self {
        self.consider.only_realized = false;
        self
    }

    // --- Axis 3: what to report -----------------------------------------------------------------

    /// How much to report.
    pub fn detail(mut self, detail: Detail) -> Self {
        self.detail = detail;
        self
    }

    /// Refuse to answer below this fidelity.
    ///
    /// ⚠ A query that *needs* real geometry says so rather than silently accepting a coarse answer.
    pub fn min_fidelity(mut self, fidelity: Fidelity) -> Self {
        self.min_fidelity = fidelity;
        self
    }

    /// What this query would report at the given fidelity.
    ///
    /// ⚠ **Never more than exists.** Asking for triangles from a world of boxes returns colliders,
    /// labelled as colliders — not triangles that were invented to satisfy the request.
    pub fn achieved_detail(&self, fidelity: Fidelity) -> Detail {
        self.detail.min(Detail::available_at(fidelity))
    }

    /// Can this query be answered at all at that fidelity?
    pub fn is_answerable_at(&self, fidelity: Fidelity) -> bool {
        fidelity >= self.min_fidelity
    }

    /// The filters, for a caller running the trace.
    pub fn consider(&self) -> &Consider {
        &self.consider
    }

    /// What is being traced, if anything.
    pub fn trace(&self) -> Option<&Trace> {
        self.trace.as_ref()
    }

    // --- terminals --------------------------------------------------------------------------------

    /// Everything the trace meets that survives the filters, nearest first.
    ///
    /// ⚠ Returns empty rather than failing when the query is unanswerable at this fidelity: *"I
    /// cannot see that yet"* is a real answer, and the caller distinguishes it with
    /// [`Query::is_answerable_at`].
    pub fn all(&self, geometry: &CoarseGeometry, fidelity: Fidelity) -> Vec<Hit> {
        if !self.is_answerable_at(fidelity) {
            return Vec::new();
        }
        let raw = match &self.trace {
            Some(Trace::Ray {
                origin,
                direction,
                range,
            }) => geometry.raycast_all(*origin, *direction, *range),
            Some(Trace::Segment { from, to }) => {
                let d = *to - *from;
                geometry.raycast_all(*from, d, d.length())
            }
            _ => Vec::new(),
        };
        raw.into_iter()
            .filter(|h| {
                let owner = geometry.get(h.collider).map(|c| c.owner);
                let tags = geometry.tags_at(h);
                owner.is_some_and(|o| self.consider.admits(o, None, &tags))
            })
            .collect()
    }

    /// The first thing the trace meets.
    pub fn first(&self, geometry: &CoarseGeometry, fidelity: Fidelity) -> Option<Hit> {
        self.all(geometry, fidelity).into_iter().next()
    }

    /// Did the trace meet anything at all?
    pub fn any(&self, geometry: &CoarseGeometry, fidelity: Fidelity) -> bool {
        self.first(geometry, fidelity).is_some()
    }

    /// How many things the trace met.
    pub fn count(&self, geometry: &CoarseGeometry, fidelity: Fidelity) -> usize {
        self.all(geometry, fidelity).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Collider;

    fn oid(n: &str) -> ObjectId {
        ObjectId::derived("actor", n)
    }

    fn world() -> CoarseGeometry {
        let mut g = CoarseGeometry::new();
        g.add(Collider::new(
            oid("near"),
            Aabb::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 4.0, 4.0)),
        ));
        g.add(
            Collider::new(
                oid("far"),
                Aabb::new(Vec3::new(8.0, 0.0, 0.0), Vec3::new(9.0, 4.0, 4.0)),
            )
            .tagged(oid("Surface.Portalable")),
        );
        g
    }

    #[test]
    fn the_coherence_guard_is_on_without_being_asked_for() {
        // ⚠ Default-on is the whole point: forgetting to think about forecasts yields a *narrower*
        // answer rather than a wrong one.
        assert!(Query::new().consider().only_realized);
        assert!(!Query::new().including_forecasts().consider().only_realized);
    }

    #[test]
    fn opting_into_forecasts_is_a_named_call_a_reader_can_grep_for() {
        // Every hook that reasons about content which may never exist has said so, in one place.
        let q = Query::new().including_forecasts();
        assert!(!q.consider().only_realized);
    }

    #[test]
    fn the_three_axes_compose_independently() {
        let q = Query::new()
            .ray(Vec3::ZERO, Vec3::X, 20.0)
            .exclude_self(oid("me"))
            .only_kind(oid("Door"))
            .with_tag(oid("Surface.Portalable"))
            .detail(Detail::Instance)
            .min_fidelity(Fidelity::Hull);

        assert!(matches!(q.trace(), Some(Trace::Ray { .. })));
        assert_eq!(q.consider().exclude, vec![oid("me")]);
        assert_eq!(q.consider().only_kinds, vec![oid("Door")]);
        assert_eq!(q.consider().with_tags, vec![oid("Surface.Portalable")]);
        assert!(!q.is_answerable_at(Fidelity::Envelope));
        assert!(q.is_answerable_at(Fidelity::Geometry));
    }

    #[test]
    fn asking_for_more_detail_than_exists_returns_what_exists() {
        // ⚠ Detail is what you ASK for; fidelity is what EXISTS. The answer is labelled, never padded.
        let q = Query::new().detail(Detail::Triangle);
        assert_eq!(q.achieved_detail(Fidelity::Envelope), Detail::Scope);
        assert_eq!(q.achieved_detail(Fidelity::Hull), Detail::Instance);
        assert_eq!(q.achieved_detail(Fidelity::Geometry), Detail::Triangle);
    }

    #[test]
    fn a_query_that_needs_real_geometry_says_so_rather_than_accepting_less() {
        let q = Query::new()
            .ray(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 20.0)
            .min_fidelity(Fidelity::Geometry);
        assert!(
            q.all(&world(), Fidelity::Envelope).is_empty(),
            "unanswerable is an empty answer, not a coarse guess"
        );
        assert!(!q.all(&world(), Fidelity::Geometry).is_empty());
    }

    #[test]
    fn exclusion_removes_a_hit_the_trace_really_made() {
        let g = world();
        let base = Query::new().ray(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 20.0);
        assert_eq!(base.count(&g, Fidelity::Geometry), 2);

        let filtered = Query::new()
            .ray(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 20.0)
            .exclude([oid("near")]);
        assert_eq!(filtered.count(&g, Fidelity::Geometry), 1);
        assert_eq!(
            filtered
                .first(&g, Fidelity::Geometry)
                .and_then(|h| g.get(h.collider).map(|c| c.owner)),
            Some(oid("far")),
            "excluding the nearer thing reveals the one behind it"
        );
    }

    #[test]
    fn a_tag_filter_selects_without_naming_anything() {
        // ⚠ Why filters are declarative: "anything portalable" keeps working as a project adds its
        // twelfth portalable material, and it survives into TypeScript and the palette.
        let g = world();
        let q = Query::new()
            .ray(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 20.0)
            .with_tag(oid("Surface.Portalable"));
        assert_eq!(q.count(&g, Fidelity::Geometry), 1);
    }

    #[test]
    fn without_tag_is_not_merely_the_absence_of_with_tag() {
        let g = world();
        let q = Query::new()
            .ray(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 20.0)
            .without_tag(oid("Surface.Portalable"));
        assert_eq!(q.count(&g, Fidelity::Geometry), 1);
        assert_eq!(
            q.first(&g, Fidelity::Geometry)
                .and_then(|h| g.get(h.collider).map(|c| c.owner)),
            Some(oid("near"))
        );
    }

    #[test]
    fn only_and_exclude_can_both_apply_and_exclude_wins() {
        // A contradiction resolves toward the narrower answer — the conservative direction.
        let g = world();
        let q = Query::new()
            .ray(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 20.0)
            .only([oid("near")])
            .exclude([oid("near")]);
        assert_eq!(q.count(&g, Fidelity::Geometry), 0);
    }

    #[test]
    fn terminals_agree_with_each_other() {
        let g = world();
        let q = Query::new().ray(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 20.0);
        assert_eq!(
            q.count(&g, Fidelity::Geometry),
            q.all(&g, Fidelity::Geometry).len()
        );
        assert_eq!(
            q.any(&g, Fidelity::Geometry),
            q.first(&g, Fidelity::Geometry).is_some()
        );
    }

    #[test]
    fn hits_come_back_nearest_first() {
        let g = world();
        let hits = Query::new()
            .ray(Vec3::new(0.0, 1.0, 1.0), Vec3::X, 20.0)
            .all(&g, Fidelity::Geometry);
        assert!(hits[0].distance < hits[1].distance);
    }
}
