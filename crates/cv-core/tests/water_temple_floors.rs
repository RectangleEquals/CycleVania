//! **Test C — `OcarinaOfTime/WaterTemple`, floors-only form.**
//!
//! ⚠ The real thing needs world state (the water level), which arrives post-gate. **This form fixes
//! the water level and tests only the geometry it leaves behind**: a central pillar spanning five
//! floors, side rooms reachable from particular floors, and one route that must thread three of them.
//!
//! # Why run it twenty-five milestones early
//!
//! Because **if the `Floor` scope cannot carry a *static* multi-floor dungeon, nothing later will
//! rescue it** — and finding that out here costs one milestone instead of a track. The `classic`
//! preset and the `cyclic-wing` scenario both assume this works.
//!
//! ⚠ **This is a plain Rust integration test, not a scenario run.** There is no way to *author* it
//! yet: the scenario runner and the editor are both later. The geometry is constructed against the
//! core directly, and the scenario milestone adopts it as a real project once both exist. Writing it
//! twice is the intended cost of finding a structural failure this early.

use cv_core::floor::detect_floors;
use cv_core::intra::{partition_floors, EdgeSource, IntraSpace};
use cv_core::{
    CoarseGeometry, Collider, Handle, Location, LocationId, MissionEdge, MissionGraph, Node,
    NodeGraph, NodeKind, NodeState, ObjectId, Rule, Solver,
};
use cv_determinism::{Aabb, Rng, Vec3};
use std::collections::{BTreeMap, BTreeSet};

fn oid(n: &str) -> ObjectId {
    ObjectId::derived("actor", n)
}
fn unlock(n: &str) -> ObjectId {
    ObjectId::derived("unlock", n)
}

/// Five stacked storeys of the central pillar, four metres apart.
const STOREYS: usize = 5;
const STOREY_HEIGHT: f64 = 4.0;

/// The temple: a central pillar of five storeys, plus four side rooms.
struct Temple {
    graph: NodeGraph,
    /// The pillar's floors, ground first.
    pillar: Vec<Handle<Node>>,
    /// Side rooms, each a single-floor Space.
    side: Vec<Handle<Node>>,
    pillar_space: Handle<Node>,
}

fn temple() -> Temple {
    let mut g = NodeGraph::new(1.0, 1);
    let reach = g.add_child(g.root(), "water_temple").unwrap();
    let area = g.add_child(reach, "interior").unwrap();

    let pillar_space = g.add_child(area, "central_pillar").unwrap();
    let pillar: Vec<_> = (0..STOREYS)
        .map(|i| g.add_child(pillar_space, format!("storey_{i}")).unwrap())
        .collect();

    // Four side rooms, each single-level — the no-regression half of the fixture.
    let mut side = Vec::new();
    for i in 0..4 {
        let s = g.add_child(area, format!("side_{i}")).unwrap();
        side.push(g.add_child(s, format!("side_{i}_floor")).unwrap());
    }

    for h in g.walk() {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(120.0)))
            .unwrap();
        g.advance(h, NodeState::Realized).unwrap();
    }
    Temple {
        graph: g,
        pillar,
        side,
        pillar_space,
    }
}

/// The pillar's collision: five slabs, stacked.
fn pillar_geometry() -> CoarseGeometry {
    let mut geo = CoarseGeometry::new();
    for i in 0..STOREYS {
        let y = i as f64 * STOREY_HEIGHT;
        geo.add(Collider::new(
            oid(&format!("storey_{i}")),
            Aabb::new(Vec3::new(0.0, y, 0.0), Vec3::new(12.0, y + 1.0, 12.0)),
        ));
    }
    geo
}

#[test]
fn the_pillar_partitions_into_five_floors_from_geometry_alone() {
    let surfaces = detect_floors(&pillar_geometry(), 50.0);
    let bands = partition_floors(&surfaces, 0.5);
    assert_eq!(
        bands.len(),
        STOREYS,
        "five slabs, four metres apart, are five floors"
    );
}

#[test]
fn a_single_level_side_room_is_unchanged_by_any_of_this() {
    let t = temple();
    for s in &t.side {
        assert_eq!(t.graph.node(*s).unwrap().kind(), NodeKind::Floor);
        let space = t
            .graph
            .scope_of(*s, NodeKind::Space)
            .expect("a floor sits in a Space");
        assert_eq!(
            t.graph.descendants_of(space).len(),
            1,
            "a single-level room has exactly one floor and gains nothing else"
        );
    }
}

#[test]
fn falling_down_the_pillar_is_derived_and_one_way() {
    // ⚠ The gravity unification. Nobody authors these edges; they exist because support ends. In the
    // real temple this is how you get *down* the pillar without the longshot.
    let t = temple();
    let surfaces = detect_floors(&pillar_geometry(), 50.0);
    let by_floor: BTreeMap<_, _> = t
        .pillar
        .iter()
        .copied()
        .zip(surfaces.iter().copied())
        .collect();

    let mut sub = IntraSpace::new(t.pillar.clone());
    sub.derive_falls(&by_floor);

    let nothing = BTreeSet::new();
    // From the top you can reach every floor below, one drop at a time.
    assert!(
        sub.all_accessible_from(t.pillar[STOREYS - 1], &nothing),
        "gravity alone gets you all the way down"
    );
    // And nothing gets you back up.
    assert_eq!(
        sub.accessible_from(t.pillar[0], &nothing).len(),
        1,
        "the ground floor is a dead end without a way up"
    );
}

#[test]
fn a_route_threads_three_storeys_and_needs_the_traversal_that_reaches_them() {
    // The shape the temple is famous for: side rooms hanging off particular storeys, and one route
    // that has to visit three of them.
    let t = temple();
    let longshot = unlock("longshot");

    let mut sub = IntraSpace::new(t.pillar.clone());
    // Climbing the pillar needs the longshot at every storey.
    for i in 0..STOREYS - 1 {
        sub.connect(
            t.pillar[i],
            t.pillar[i + 1],
            EdgeSource::Assisted {
                via: t.pillar[i],
                requires: [longshot].into_iter().collect(),
            },
        );
    }
    let surfaces = detect_floors(&pillar_geometry(), 50.0);
    let by_floor: BTreeMap<_, _> = t
        .pillar
        .iter()
        .copied()
        .zip(surfaces.iter().copied())
        .collect();
    sub.derive_falls(&by_floor);

    let nothing = BTreeSet::new();
    assert_eq!(
        sub.accessible_from(t.pillar[0], &nothing).len(),
        1,
        "without the longshot the ground floor goes nowhere"
    );
    let held: BTreeSet<_> = [longshot].into_iter().collect();
    assert!(
        sub.all_accessible_from(t.pillar[0], &held),
        "with it, all five storeys are accessible"
    );
}

/// The mission graph over the temple, at **Floor** granularity.
fn temple_mission(t: &Temple, longshot: ObjectId) -> MissionGraph {
    let mut m = MissionGraph::new(t.pillar[0]);
    // Climbing needs the longshot.
    for i in 0..STOREYS - 1 {
        m.add_edge(MissionEdge::gated(
            t.pillar[i],
            t.pillar[i + 1],
            Rule::has(longshot),
        ));
    }
    // Side rooms hang off particular storeys — 0, 1, 2 and 4.
    for (k, storey) in [0usize, 1, 2, 4].into_iter().enumerate() {
        m.add_edge(MissionEdge::open(t.pillar[storey], t.side[k]));
    }
    m.set_goal(t.side[3]);

    for (id, h) in t.pillar.iter().chain(t.side.iter()).enumerate() {
        m.add_location(LocationId(id as u32), Location { scope: *h, slot: 0 });
    }
    m
}

#[test]
fn the_longshot_cannot_be_placed_where_it_would_be_needed_to_reach_itself() {
    // ⚠ The claim this whole milestone exists to support. The side room off storey 4 is only
    // reachable by climbing, and climbing needs the longshot. A Space-granular solve would call
    // "somewhere in the pillar" legal; a floor-granular one refuses.
    let t = temple();
    let longshot = unlock("longshot");
    let item = oid("longshot_item");
    let m = temple_mission(&t, longshot);
    let g = NodeGraph::new(1.0, 1);

    let gated: Vec<Handle<Node>> = t.pillar[1..].iter().copied().chain([t.side[3]]).collect();

    for seed in 0..40u64 {
        let solution = Solver::new(&g)
            .with_grant(item, longshot)
            .fill(&m, &[item], &Rng::new(seed))
            .expect("the temple is solvable");
        let (loc, _) = solution
            .placements
            .iter()
            .find(|(_, i)| **i == item)
            .expect("placed");
        let scope = m
            .locations()
            .find(|(id, _)| id == loc)
            .map(|(_, l)| l.scope)
            .expect("scoped");
        assert!(
            !gated.contains(&scope),
            "seed {seed}: the longshot landed on a floor that needs the longshot"
        );
    }
}

#[test]
fn the_spheres_climb_with_the_storeys() {
    // A five-storey climb behind one gate is not five spheres — but it *is* more than one, and the
    // storeys must not all collapse into the ground floor's sphere.
    let t = temple();
    let longshot = unlock("longshot");
    let item = oid("longshot_item");
    let m = temple_mission(&t, longshot);

    let placements: BTreeMap<LocationId, ObjectId> = [(LocationId(0), item)].into_iter().collect();
    let grants: BTreeMap<ObjectId, BTreeSet<ObjectId>> = [(item, [longshot].into_iter().collect())]
        .into_iter()
        .collect();
    let swept = m.sweep(&BTreeSet::new(), &placements, &grants);

    let ground = swept.sphere_of(t.pillar[0]).expect("ground is reached");
    let top = swept
        .sphere_of(t.pillar[STOREYS - 1])
        .expect("the top is reached");
    assert!(
        top > ground,
        "the top storey opens later than the ground floor ({top} vs {ground})"
    );
    assert!(
        swept.scopes.len() >= STOREYS,
        "every storey is a distinct place the ladder can name"
    );
}

#[test]
fn the_temple_is_a_multi_floor_dungeon_the_scope_can_actually_carry() {
    // The summary assertion, and the reason this test runs now rather than post-gate: a static
    // multi-floor dungeon must be expressible, solvable and reproducible with what exists today.
    let t = temple();
    let longshot = unlock("longshot");
    let item = oid("longshot_item");
    let m = temple_mission(&t, longshot);
    let g = NodeGraph::new(1.0, 1);

    let solve = |seed: u64| {
        Solver::new(&g)
            .with_grant(item, longshot)
            .fill(&m, &[item], &Rng::new(seed))
            .expect("solvable")
            .placements
    };
    assert_eq!(solve(3), solve(3), "the same seed gives the same temple");
    assert_eq!(
        t.graph.descendants_of(t.pillar_space).len(),
        STOREYS,
        "the pillar is one Space of five Floors, not five Spaces"
    );
}
