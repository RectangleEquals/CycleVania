//! **L2d: the directed subgraph inside a Space.**
//!
//! [`crate::floor`] produces standable *geometry*. This produces the *topology* over it — which floors
//! are mutually accessible from which, and by what.
//!
//! > Floor collision at **L2a** is **geometry**. The `Floor` scope at **L2d** is **topology**. The first
//! > produces what the second partitions.
//!
//! # Intra-space traversal is not a lesser thing than inter-space traversal
//!
//! It is the same question at a finer grain, and it runs on the same machinery. A tower's balcony is
//! gated from its ground floor exactly as one room is gated from another — and if the solver can only
//! reason at Space granularity, a key *"in the tower"* is placeable on the balcony you cannot reach
//! without it. Only a floor-granular solve refuses that.
//!
//! # The four edge sources
//!
//! | Source | What supplies it |
//! |---|---|
//! | ramp · staircase · jump pad · elevator · moving platform · portal | a **Spatial** carrying a traversal |
//! | rope target · ledge grab | a Spatial **plus** something the occupant holds |
//! | double jump · glide · dash | **an ability alone** — no content at all |
//! | falling · sinking · losing support | **geometry alone** — a derived one-way `DOWN` edge |
//!
//! ⚠ **The fourth source is why this cannot be authored content.** Gravity is not a thing a developer
//! places; it is what happens when support ends. So the subgraph has to be *derived*, and a Space's
//! `floor_count` can only ever be a **preference** the solver may miss under scarcity.
//!
//! # The gravity unification
//!
//! *Unsupported means gravity applies, and gravity is a derived one-way `DOWN` edge.* Falling off a
//! ledge, sinking through water and a hover effect expiring are **the same edge** — and **what lies
//! below decides the outcome**: another floor is a traversal, a lava floor is harm, a lake bottom is a
//! traversal to somewhere lower.
//!
//! ⚠ Collapsing those three into one edge is what stops the solver needing a special case per hazard.
//! `supports()` decides what happens on arrival; the edge itself is pure geometry.

use crate::arena::Handle;
use crate::floor::FloorSurface;
use crate::node::Node;
use crate::object::ObjectId;
use cv_determinism::{math, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// Why one floor can be left for another.
///
/// ⚠ **Carried on the edge, not looked up later.** The solver's dependency walk has to see every
/// requirement an edge imposes; an edge that knew its own reason but not its own cost would make
/// *"what does this route need?"* unanswerable without re-deriving the world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EdgeSource {
    /// A Spatial carrying a traversal — a staircase, a ramp, a lift, a portal.
    Traversal {
        /// The Spatial that supplies it.
        via: Handle<Node>,
    },
    /// A Spatial plus something the occupant must hold — a rope target, a ledge grab.
    Assisted {
        /// The Spatial that supplies the anchor.
        via: Handle<Node>,
        /// What the occupant must hold to use it.
        requires: BTreeSet<ObjectId>,
    },
    /// An ability alone — a double jump, a glide, a dash. **No content is involved.**
    Ability {
        /// What the occupant must hold.
        requires: BTreeSet<ObjectId>,
    },
    /// **Gravity.** Derived from geometry alone, and always one-way.
    ///
    /// ⚠ Falling off a ledge, sinking through water and a hover expiring are all *this*. What is
    /// below decides whether arriving is fine, harmful, or merely lower.
    Fall,
}

impl EdgeSource {
    /// What an occupant must hold to use this, if anything.
    pub fn requires(&self) -> &BTreeSet<ObjectId> {
        static EMPTY: BTreeSet<ObjectId> = BTreeSet::new();
        match self {
            EdgeSource::Assisted { requires, .. } | EdgeSource::Ability { requires } => requires,
            EdgeSource::Traversal { .. } | EdgeSource::Fall => &EMPTY,
        }
    }

    /// Is this edge one-way by nature?
    ///
    /// ⚠ Only gravity is. Everything else is one-way or not according to what supplies it, which is a
    /// property of the content rather than of the edge kind.
    pub fn is_inherently_one_way(&self) -> bool {
        matches!(self, EdgeSource::Fall)
    }
}

/// One directed edge between floors of a Space.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FloorEdge {
    /// Where it starts.
    pub from: Handle<Node>,
    /// Where it lands.
    pub to: Handle<Node>,
    /// What makes it possible.
    pub source: EdgeSource,
}

/// The directed subgraph over the floors of one Space.
///
/// ⚠ **Rebuilt whenever a traversal Spatial commits or moves**, so it stays cheap on purpose: placing a
/// staircase changes which floors are mutually accessible, and every sphere and gate decision
/// downstream reads the result. It is not a one-time product of L2d.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IntraSpace {
    floors: Vec<Handle<Node>>,
    edges: Vec<FloorEdge>,
}

impl IntraSpace {
    /// An empty subgraph over a known set of floors.
    pub fn new(floors: impl IntoIterator<Item = Handle<Node>>) -> Self {
        let mut floors: Vec<Handle<Node>> = floors.into_iter().collect();
        floors.sort();
        floors.dedup();
        IntraSpace {
            floors,
            edges: Vec::new(),
        }
    }

    /// Add a directed edge.
    pub fn connect(&mut self, from: Handle<Node>, to: Handle<Node>, source: EdgeSource) {
        self.edges.push(FloorEdge { from, to, source });
    }

    /// Add both directions of a two-way traversal.
    ///
    /// ⚠ Kept explicit rather than defaulted: a staircase is two-way and a jump pad is not, and
    /// guessing from the content type is how a one-way commit becomes invisible.
    pub fn connect_both(&mut self, a: Handle<Node>, b: Handle<Node>, source: EdgeSource) {
        self.connect(a, b, source.clone());
        self.connect(b, a, source);
    }

    /// **The gravity pass** — derive one-way `DOWN` edges from geometry alone.
    ///
    /// For each floor, the nearest floor **strictly below** it that overlaps in plan gets a `Fall`
    /// edge. That single rule covers falling off a ledge, sinking, and a hover expiring, because all
    /// three are *support ending* and none of them care why.
    ///
    /// ⚠ **What lies below decides the outcome, not this pass.** The edge is created regardless; a
    /// lava floor is still a floor, and `supports()` is what makes arriving there harmful.
    pub fn derive_falls(&mut self, floors: &BTreeMap<Handle<Node>, FloorSurface>) {
        for (&from, upper) in floors {
            let mut best: Option<(Handle<Node>, f64)> = None;
            for (&to, lower) in floors {
                if to == from || lower.patch.max.y >= upper.patch.max.y {
                    continue;
                }
                if !overlaps_in_plan(upper, lower) {
                    continue;
                }
                let drop = upper.patch.max.y - lower.patch.max.y;
                if best.is_none_or(|(_, d)| drop < d) {
                    best = Some((to, drop));
                }
            }
            if let Some((to, _)) = best {
                self.connect(from, to, EdgeSource::Fall);
            }
        }
    }

    /// The floors in this Space, sorted.
    pub fn floors(&self) -> &[Handle<Node>] {
        &self.floors
    }

    /// Every edge, in insertion order.
    pub fn edges(&self) -> &[FloorEdge] {
        &self.edges
    }

    /// Which floors are reachable from `start` while holding `held`.
    ///
    /// ⚠ **Directed, so this is not symmetric** — and that asymmetry is the entire point. A fall takes
    /// you down and does not bring you back, which is what makes an unreachable balcony detectable
    /// rather than assumed away.
    pub fn accessible_from(
        &self,
        start: Handle<Node>,
        held: &BTreeSet<ObjectId>,
    ) -> BTreeSet<Handle<Node>> {
        let mut seen = BTreeSet::new();
        let mut queue = vec![start];
        seen.insert(start);
        while let Some(at) = queue.pop() {
            for e in self.edges.iter().filter(|e| e.from == at) {
                if !e.source.requires().is_subset(held) {
                    continue;
                }
                if seen.insert(e.to) {
                    queue.push(e.to);
                }
            }
        }
        seen
    }

    /// Is every floor reachable from `start` with `held`?
    ///
    /// This is the question a Space's `floor_count` preference is really about: several floors that
    /// nothing connects are several *unreachable* floors.
    pub fn all_accessible_from(&self, start: Handle<Node>, held: &BTreeSet<ObjectId>) -> bool {
        self.accessible_from(start, held).len() == self.floors.len()
    }

    /// **Collapse every floor into one** — the mutation used to falsify the whole scope.
    ///
    /// ⚠ If a solve produces the same answer against this as against the real subgraph, the `Floor`
    /// scope is inert: built, maintained, and consulted by nothing. That is the single most valuable
    /// assertion available here, because it is the only one that can fail while every other test
    /// passes.
    pub fn collapsed(&self) -> IntraSpace {
        let one = self.floors.first().copied();
        IntraSpace {
            floors: one.into_iter().collect(),
            edges: Vec::new(),
        }
    }
}

/// Do two floor patches overlap when viewed from above?
fn overlaps_in_plan(a: &FloorSurface, b: &FloorSurface) -> bool {
    let gap = |amin: f64, amax: f64, bmin: f64, bmax: f64| amax < bmin || bmax < amin;
    !gap(a.patch.min.x, a.patch.max.x, b.patch.min.x, b.patch.max.x)
        && !gap(a.patch.min.z, a.patch.max.z, b.patch.min.z, b.patch.max.z)
}

/// Partition standable geometry into floors by elevation band.
///
/// ⚠ **Derived, never authored.** Patches within `tolerance` of each other in height and overlapping
/// in plan belong to the same floor. A single-level room yields exactly one, so nothing changes for it
/// — which is the no-regression property the whole tier depends on.
pub fn partition_floors(surfaces: &[FloorSurface], tolerance: f64) -> Vec<Vec<usize>> {
    let mut bands: Vec<Vec<usize>> = Vec::new();
    for (i, s) in surfaces.iter().enumerate() {
        let found = bands.iter_mut().find(|band| {
            band.iter()
                .any(|&j| math::abs(surfaces[j].patch.max.y - s.patch.max.y) <= tolerance)
        });
        match found {
            Some(band) => band.push(i),
            None => bands.push(vec![i]),
        }
    }
    bands
}

/// Which way is down, for the fall pass.
pub const DOWN: Vec3 = Vec3 {
    x: 0.0,
    y: -1.0,
    z: 0.0,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floor::detect_floors;
    use crate::geometry::{CoarseGeometry, Collider};
    use crate::node::{NodeGraph, NodeKind, NodeState};
    use cv_determinism::{Aabb, Vec3};

    fn oid(n: &str) -> ObjectId {
        ObjectId::derived("actor", n)
    }

    /// A Space with `n` floors under it.
    fn space_with_floors(n: usize) -> (NodeGraph, Handle<Node>, Vec<Handle<Node>>) {
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "reach").unwrap();
        let area = g.add_child(reach, "area").unwrap();
        let space = g.add_child(area, "tower").unwrap();
        let floors: Vec<_> = (0..n)
            .map(|i| g.add_child(space, format!("floor_{i}")).unwrap())
            .collect();
        for h in g.walk() {
            g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(40.0)))
                .unwrap();
            g.advance(h, NodeState::Realized).unwrap();
        }
        (g, space, floors)
    }

    #[test]
    fn a_space_contains_floors_which_contain_spatials() {
        let (mut g, space, floors) = space_with_floors(2);
        assert_eq!(g.node(space).unwrap().kind(), NodeKind::Space);
        assert_eq!(g.node(floors[0]).unwrap().kind(), NodeKind::Floor);
        let ledge = g.add_child(floors[0], "ledge").unwrap();
        assert_eq!(g.node(ledge).unwrap().kind(), NodeKind::Spatial);
    }

    #[test]
    fn a_single_level_room_has_exactly_one_floor() {
        // The no-regression guard: nothing changes for a room that was never multi-floor.
        let mut geo = CoarseGeometry::new();
        geo.add(Collider::new(
            oid("ground"),
            Aabb::new(Vec3::ZERO, Vec3::new(8.0, 1.0, 8.0)),
        ));
        let surfaces = detect_floors(&geo, 50.0);
        assert_eq!(partition_floors(&surfaces, 0.5).len(), 1);
    }

    #[test]
    fn floors_are_partitioned_by_elevation() {
        let mut geo = CoarseGeometry::new();
        geo.add(Collider::new(
            oid("ground"),
            Aabb::new(Vec3::ZERO, Vec3::new(8.0, 1.0, 8.0)),
        ));
        geo.add(Collider::new(
            oid("balcony"),
            Aabb::new(Vec3::new(0.0, 6.0, 0.0), Vec3::new(4.0, 7.0, 4.0)),
        ));
        let surfaces = detect_floors(&geo, 50.0);
        assert_eq!(
            partition_floors(&surfaces, 0.5).len(),
            2,
            "two elevations, two floors"
        );
    }

    #[test]
    fn a_fall_is_derived_from_geometry_and_is_one_way() {
        // ⚠ Gravity is not authored content. Nobody places this edge; it exists because support ends.
        let (_, _, floors) = space_with_floors(2);
        let mut geo = CoarseGeometry::new();
        geo.add(Collider::new(
            oid("ground"),
            Aabb::new(Vec3::ZERO, Vec3::new(8.0, 1.0, 8.0)),
        ));
        geo.add(Collider::new(
            oid("balcony"),
            Aabb::new(Vec3::new(0.0, 6.0, 0.0), Vec3::new(4.0, 7.0, 4.0)),
        ));
        let surfaces = detect_floors(&geo, 50.0);
        let by_floor: BTreeMap<_, _> = [(floors[0], surfaces[0]), (floors[1], surfaces[1])]
            .into_iter()
            .collect();

        let mut sub = IntraSpace::new(floors.clone());
        sub.derive_falls(&by_floor);

        let held = BTreeSet::new();
        // Down is free.
        assert!(sub.accessible_from(floors[1], &held).contains(&floors[0]));
        // Up is not.
        assert!(!sub.accessible_from(floors[0], &held).contains(&floors[1]));
    }

    #[test]
    fn a_staircase_opens_the_balcony_in_both_directions() {
        let (_, _, floors) = space_with_floors(2);
        let mut sub = IntraSpace::new(floors.clone());
        sub.connect_both(
            floors[0],
            floors[1],
            EdgeSource::Traversal {
                via: floors[0], // stands in for the Spatial carrying the stair
            },
        );
        let held = BTreeSet::new();
        assert!(sub.all_accessible_from(floors[0], &held));
        assert!(sub.all_accessible_from(floors[1], &held));
    }

    #[test]
    fn an_ability_alone_can_be_an_edge() {
        // ⚠ No content is involved. A double jump connects two floors with nothing placed between.
        let (_, _, floors) = space_with_floors(2);
        let dash = ObjectId::derived("unlock", "double_jump");
        let mut sub = IntraSpace::new(floors.clone());
        sub.connect(
            floors[0],
            floors[1],
            EdgeSource::Ability {
                requires: [dash].into_iter().collect(),
            },
        );

        assert!(!sub.all_accessible_from(floors[0], &BTreeSet::new()));
        assert!(sub.all_accessible_from(floors[0], &[dash].into_iter().collect()));
    }

    #[test]
    fn an_assisted_edge_needs_both_the_anchor_and_the_thing_held() {
        let (_, _, floors) = space_with_floors(2);
        let hook = ObjectId::derived("unlock", "tether");
        let mut sub = IntraSpace::new(floors.clone());
        sub.connect(
            floors[0],
            floors[1],
            EdgeSource::Assisted {
                via: floors[0],
                requires: [hook].into_iter().collect(),
            },
        );
        assert!(!sub.all_accessible_from(floors[0], &BTreeSet::new()));
        assert!(sub.all_accessible_from(floors[0], &[hook].into_iter().collect()));
    }

    #[test]
    fn collapsing_the_floors_destroys_the_distinction() {
        // The falsification. If a solve reads the same from both, the scope is inert.
        let (_, _, floors) = space_with_floors(3);
        let mut sub = IntraSpace::new(floors.clone());
        sub.connect(floors[2], floors[0], EdgeSource::Fall);

        assert_eq!(sub.floors().len(), 3);
        assert!(!sub.all_accessible_from(floors[0], &BTreeSet::new()));

        let flat = sub.collapsed();
        assert_eq!(flat.floors().len(), 1);
        assert!(
            flat.all_accessible_from(floors[0], &BTreeSet::new()),
            "collapsed, everything is trivially accessible — which is exactly the lie"
        );
    }

    #[test]
    fn the_subgraph_rebuilds_when_a_traversal_moves() {
        // ⚠ Not a one-time product of L2d: placing a staircase changes what is mutually accessible.
        let (_, _, floors) = space_with_floors(3);
        let held = BTreeSet::new();

        let mut before = IntraSpace::new(floors.clone());
        before.connect_both(
            floors[0],
            floors[1],
            EdgeSource::Traversal { via: floors[0] },
        );
        assert!(!before.all_accessible_from(floors[0], &held));

        // The stair is moved to serve the top floor instead.
        let mut after = IntraSpace::new(floors.clone());
        after.connect_both(
            floors[0],
            floors[1],
            EdgeSource::Traversal { via: floors[0] },
        );
        after.connect_both(
            floors[1],
            floors[2],
            EdgeSource::Traversal { via: floors[1] },
        );
        assert!(after.all_accessible_from(floors[0], &held));
    }

    #[test]
    fn a_fall_lands_on_the_nearest_floor_below_not_the_lowest() {
        let (_, _, floors) = space_with_floors(3);
        let mut geo = CoarseGeometry::new();
        for (i, y) in [0.0, 4.0, 8.0].into_iter().enumerate() {
            geo.add(Collider::new(
                oid(&format!("f{i}")),
                Aabb::new(Vec3::new(0.0, y, 0.0), Vec3::new(4.0, y + 1.0, 4.0)),
            ));
        }
        let surfaces = detect_floors(&geo, 50.0);
        let by_floor: BTreeMap<_, _> = floors
            .iter()
            .copied()
            .zip(surfaces.iter().copied())
            .collect();

        let mut sub = IntraSpace::new(floors.clone());
        sub.derive_falls(&by_floor);

        let from_top: Vec<_> = sub.edges().iter().filter(|e| e.from == floors[2]).collect();
        assert_eq!(from_top.len(), 1, "one fall per floor");
        assert_eq!(from_top[0].to, floors[1], "you land on the next one down");
    }

    #[test]
    fn floors_that_do_not_overlap_in_plan_do_not_fall_into_each_other() {
        let (_, _, floors) = space_with_floors(2);
        let mut geo = CoarseGeometry::new();
        geo.add(Collider::new(
            oid("a"),
            Aabb::new(Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)),
        ));
        geo.add(Collider::new(
            oid("b"),
            Aabb::new(Vec3::new(50.0, 6.0, 0.0), Vec3::new(54.0, 7.0, 4.0)),
        ));
        let surfaces = detect_floors(&geo, 50.0);
        let by_floor: BTreeMap<_, _> = [(floors[0], surfaces[0]), (floors[1], surfaces[1])]
            .into_iter()
            .collect();
        let mut sub = IntraSpace::new(floors.clone());
        sub.derive_falls(&by_floor);
        assert!(sub.edges().is_empty(), "nothing is below anything here");
    }
}
