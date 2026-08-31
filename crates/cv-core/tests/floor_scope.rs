//! M05a exit criteria: the **`Floor` scope**, and proof the solver actually consults it.
//!
//! ⚠ **Testing this in a single Space proves almost nothing.** A one-room test shows the subgraph
//! *exists*. It cannot show the solver *reads* it — and the failure mode that matters is `Floor` being
//! built correctly and then silently ignored, because the mission graph, the assumed fill and the
//! sphere ladder were all written at Space granularity.
//!
//! So the shape here is:
//!
//! * **Test A** — one Space, every edge source at once.
//! * **Test B** — several Spaces with differing floor counts, and a key that must land on a *floor*.
//! * **The falsification** — collapse every `Floor` into its `Space` and re-run. **The solve must
//!   break.** If it succeeds identically, the scope is inert: built, maintained, consulted by nothing.

use cv_core::floor::detect_floors;
use cv_core::intra::{partition_floors, EdgeSource, IntraSpace};
use cv_core::{
    CoarseGeometry, Collider, EdgeSpan, Handle, Location, LocationId, MissionEdge, MissionGraph,
    Node, NodeGraph, NodeKind, NodeState, ObjectId, Rule, Solver,
};
use cv_determinism::{Aabb, Rng, Vec3};
use std::collections::{BTreeMap, BTreeSet};

fn oid(n: &str) -> ObjectId {
    ObjectId::derived("actor", n)
}
fn unlock(n: &str) -> ObjectId {
    ObjectId::derived("unlock", n)
}

/// A scope graph, its Spaces, and the Floors under each.
type World = (NodeGraph, Vec<Handle<Node>>, Vec<Vec<Handle<Node>>>);

/// A world whose rooms have the given floor counts.
fn world(floors: &[usize]) -> World {
    let mut g = NodeGraph::new(1.0, 1);
    let reach = g.add_child(g.root(), "reach").unwrap();
    let area = g.add_child(reach, "area").unwrap();
    let mut spaces = Vec::new();
    let mut per_space = Vec::new();
    for (i, count) in floors.iter().enumerate() {
        let s = g.add_child(area, format!("space_{i}")).unwrap();
        let fs: Vec<_> = (0..*count)
            .map(|k| g.add_child(s, format!("s{i}_floor{k}")).unwrap())
            .collect();
        spaces.push(s);
        per_space.push(fs);
    }
    for h in g.walk() {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(80.0)))
            .unwrap();
        g.advance(h, NodeState::Realized).unwrap();
    }
    (g, spaces, per_space)
}

// ---------------------------------------------------------------------------------------------
// Test A — one Space, every edge source
// ---------------------------------------------------------------------------------------------

#[test]
fn one_space_exercises_all_four_edge_sources() {
    let (_, _, per) = world(&[4]);
    let f = &per[0];
    let tether = unlock("tether");
    let glide = unlock("glide");

    let mut sub = IntraSpace::new(f.clone());
    // 1. a staircase — a Spatial carrying a traversal, two-way
    sub.connect_both(f[0], f[1], EdgeSource::Traversal { via: f[0] });
    // 2. a ledge grab — a Spatial plus something held
    sub.connect(
        f[1],
        f[2],
        EdgeSource::Assisted {
            via: f[1],
            requires: [tether].into_iter().collect(),
        },
    );
    // 3. a glide — an ability alone, no content
    sub.connect(
        f[2],
        f[3],
        EdgeSource::Ability {
            requires: [glide].into_iter().collect(),
        },
    );
    // 4. a fall — geometry alone, one-way
    sub.connect(f[3], f[0], EdgeSource::Fall);

    let nothing = BTreeSet::new();
    assert_eq!(
        sub.accessible_from(f[0], &nothing).len(),
        2,
        "with nothing held only the staircase is usable"
    );

    let both: BTreeSet<_> = [tether, glide].into_iter().collect();
    assert!(
        sub.all_accessible_from(f[0], &both),
        "held, the tower opens up"
    );

    // ⚠ Directed: the fall gets you down from the top, and nothing gets you back up.
    assert!(sub.accessible_from(f[3], &nothing).contains(&f[0]));
    assert!(!sub.accessible_from(f[0], &nothing).contains(&f[3]));
}

#[test]
fn floors_are_derived_from_geometry_not_declared() {
    // A tower of three slabs, and a single-level annex. The partition must find 3 and 1 from geometry
    // alone, with nobody having declared how many there are.
    let mut tower = CoarseGeometry::new();
    for (i, y) in [0.0, 5.0, 10.0].into_iter().enumerate() {
        tower.add(Collider::new(
            oid(&format!("t{i}")),
            Aabb::new(Vec3::new(0.0, y, 0.0), Vec3::new(6.0, y + 1.0, 6.0)),
        ));
    }
    assert_eq!(partition_floors(&detect_floors(&tower, 50.0), 0.5).len(), 3);

    let mut annex = CoarseGeometry::new();
    annex.add(Collider::new(
        oid("annex"),
        Aabb::new(Vec3::ZERO, Vec3::new(6.0, 1.0, 6.0)),
    ));
    assert_eq!(
        partition_floors(&detect_floors(&annex, 50.0), 0.5).len(),
        1,
        "a single-level room has exactly one, so nothing changes for it"
    );
}

// ---------------------------------------------------------------------------------------------
// Test B — several Spaces, and the falsification
// ---------------------------------------------------------------------------------------------

/// The world Test B and the falsification share.
///
/// Four Spaces: an entrance (1 floor), two wings that **differ in floor count** (3 and 2), and a
/// capstone (1). The tether key sits somewhere, and wing A's upper floors are gated on it.
struct Fixture {
    per: Vec<Vec<Handle<Node>>>,
    spaces: Vec<Handle<Node>>,
}

fn fixture() -> Fixture {
    let (_, spaces, per) = world(&[1, 3, 2, 1]);
    Fixture { per, spaces }
}

/// The mission graph at **Floor** granularity — one node per floor.
fn floor_granular(f: &Fixture, tether: ObjectId) -> MissionGraph {
    let entrance = f.per[0][0];
    let mut m = MissionGraph::new(entrance);

    m.add_edge(MissionEdge::open(entrance, f.per[1][0]));
    m.add_edge(MissionEdge::open(entrance, f.per[2][0]));

    // ⚠ Wing A's upper floors need the tether. This is the whole point: the balcony is a *floor*, and
    // it is gated, so a key placed there would sit behind itself.
    m.add_edge(MissionEdge::gated(
        f.per[1][0],
        f.per[1][1],
        Rule::has(tether),
    ));
    m.add_edge(MissionEdge::gated(
        f.per[1][1],
        f.per[1][2],
        Rule::has(tether),
    ));
    // Wing B has a staircase, so no gate.
    m.add_edge(MissionEdge::open(f.per[2][0], f.per[2][1]));

    // Both wings reconverge on the capstone. ⚠ Wing A's exit is **one-way** — you drop out of the
    // tower into the capstone and cannot climb back. Without that, the capstone is reachable through
    // ungated wing B and the tower's top floor becomes reachable *backwards*, which would make the
    // gate meaningless. (This test found that the hard way.)
    m.add_edge(MissionEdge::open(f.per[1][2], f.per[3][0]).one_way());
    m.add_edge(MissionEdge::open(f.per[2][1], f.per[3][0]));
    m.set_goal(f.per[3][0]);

    for (i, h) in f.per.iter().flatten().enumerate() {
        m.add_location(LocationId(i as u32), Location { scope: *h, slot: 0 });
    }
    m
}

/// The same world with **every floor collapsed into its Space** — the mutation.
fn space_granular(f: &Fixture, tether: ObjectId) -> MissionGraph {
    let mut m = MissionGraph::new(f.spaces[0]);
    m.add_edge(MissionEdge::open(f.spaces[0], f.spaces[1]));
    m.add_edge(MissionEdge::open(f.spaces[0], f.spaces[2]));
    // ⚠ The gate that lived *between floors of wing A* has nowhere to go. Collapsed, wing A is one
    // node, and the balcony is indistinguishable from the ground floor beneath it.
    m.add_edge(MissionEdge::gated(
        f.spaces[1],
        f.spaces[3],
        Rule::has(tether),
    ));
    m.add_edge(MissionEdge::open(f.spaces[2], f.spaces[3]));
    m.set_goal(f.spaces[3]);
    for (i, h) in f.spaces.iter().enumerate() {
        m.add_location(LocationId(i as u32), Location { scope: *h, slot: 0 });
    }
    m
}

#[test]
fn a_key_is_placed_at_floor_granularity_not_space_granularity() {
    let f = fixture();
    let tether = unlock("tether");
    let key = oid("tether_item");
    let m = floor_granular(&f, tether);

    let g = NodeGraph::new(1.0, 1);
    let solver = Solver::new(&g).with_grant(key, tether);
    let solution = solver
        .fill(&m, &[key], &Rng::new(9))
        .expect("the world is solvable");

    let (loc, _) = solution
        .placements
        .iter()
        .find(|(_, item)| **item == key)
        .expect("the key was placed");
    let scope = m
        .locations()
        .find(|(id, _)| id == loc)
        .map(|(_, l)| l.scope)
        .expect("the location has a scope");

    // ⚠ The balcony floors are gated on the very key being placed.
    assert!(
        scope != f.per[1][1] && scope != f.per[1][2],
        "the key must not land on a floor that needs it — the whole point of floor granularity"
    );
}

#[test]
fn collapsing_floors_into_spaces_destroys_the_guarantee() {
    // ⚠ **The falsification, and the single most valuable assertion in this milestone** — it is the
    // only one that can fail while every other test passes.
    //
    // The claim is not "the floor graph has more nodes"; that is trivially true and proves nothing.
    // The claim is that **the two solves permit different placements**, and that the collapsed one
    // permits an unreachable placement the floor-granular one refuses.
    let f = fixture();
    let tether = unlock("tether");
    let key = oid("tether_item");
    let g = NodeGraph::new(1.0, 1);

    let floors = floor_granular(&f, tether);
    let spaces = space_granular(&f, tether);

    // Where does each solve put the key, across many seeds?
    let placed_in = |m: &MissionGraph, seed: u64| -> Handle<Node> {
        let solution = Solver::new(&g)
            .with_grant(key, tether)
            .fill(m, &[key], &Rng::new(seed))
            .expect("solvable");
        let (loc, _) = solution
            .placements
            .iter()
            .find(|(_, i)| **i == key)
            .expect("placed");
        m.locations()
            .find(|(id, _)| id == loc)
            .map(|(_, l)| l.scope)
            .expect("scoped")
    };

    let gated_floors = [f.per[1][1], f.per[1][2]];
    let mut deep_ever_gated = false;
    let mut flat_ever_wing_a = false;
    for seed in 0..60u64 {
        if gated_floors.contains(&placed_in(&floors, seed)) {
            deep_ever_gated = true;
        }
        if placed_in(&spaces, seed) == f.spaces[1] {
            flat_ever_wing_a = true;
        }
    }

    assert!(
        !deep_ever_gated,
        "floor-granular: the key must NEVER land on a floor gated by itself"
    );
    assert!(
        flat_ever_wing_a,
        "space-granular: the collapsed solve puts the key somewhere in wing A"
    );

    // ⚠ And that is the break. "Somewhere in wing A" is a *Space*, and wing A contains two floors
    // that need the tether to enter. The collapsed solve cannot tell the ground floor from the
    // balcony, so it calls a placement legal that the floor-granular solve refuses outright.
    let nothing = BTreeSet::new();
    let open = floors.traverse(&nothing);
    assert!(
        open.contains(&f.per[1][0]) && !open.contains(&f.per[1][1]),
        "wing A's ground floor is free and its balcony is not — the distinction the collapse loses"
    );
    assert!(
        spaces.traverse(&nothing).contains(&f.spaces[1]),
        "collapsed, the whole wing reads as open, balcony included"
    );
}

#[test]
fn wings_may_differ_in_floor_count_and_still_reconverge() {
    // The genre's signature shape, crossing the new rung: fork into a 3-floor wing and a 2-floor
    // wing, rejoin at a single-floor capstone.
    let f = fixture();
    let tether = unlock("tether");
    let m = floor_granular(&f, tether);

    assert_eq!(f.per[1].len(), 3);
    assert_eq!(f.per[2].len(), 2);
    assert_eq!(f.per[0].len(), 1, "the entrance is single-level");

    let held: BTreeSet<_> = [tether].into_iter().collect();
    let open = m.traverse(&held);
    assert!(open.contains(&f.per[1][2]), "wing A tops out");
    assert!(open.contains(&f.per[2][1]), "wing B tops out");
    assert!(open.contains(&f.per[3][0]), "and they reconverge");
}

#[test]
fn a_gated_floor_is_a_different_sphere_from_its_own_ground_floor() {
    // ⚠ The sphere ladder has to count *floors*. If it counted Spaces, a balcony behind a gate would
    // share a sphere with the ground floor beneath it, and the pacing curve would be a fiction.
    let f = fixture();
    let tether = unlock("tether");
    let key = oid("tether_item");
    let m = floor_granular(&f, tether);

    let placements: BTreeMap<LocationId, ObjectId> = [(LocationId(0), key)].into_iter().collect();
    let grants: BTreeMap<ObjectId, BTreeSet<ObjectId>> = [(key, [tether].into_iter().collect())]
        .into_iter()
        .collect();
    let swept = m.sweep(&BTreeSet::new(), &placements, &grants);

    let ground = swept
        .sphere_of(f.per[1][0])
        .expect("the ground floor is reached");
    let balcony = swept
        .sphere_of(f.per[1][1])
        .expect("the balcony is reached");
    assert!(
        balcony > ground,
        "the balcony must open later than the floor below it ({balcony} vs {ground})"
    );
}

#[test]
fn no_existing_single_floor_behaviour_moved() {
    // The no-regression guard the milestone asks for, stated as a test rather than as a hope.
    let (g, spaces, per) = world(&[1, 1]);
    assert_eq!(per[0].len(), 1);
    assert_eq!(g.node(spaces[0]).unwrap().kind(), NodeKind::Space);
    assert_eq!(g.node(per[0][0]).unwrap().kind(), NodeKind::Floor);

    let m = MissionGraph::new(per[0][0]);
    assert_eq!(
        m.traverse(&BTreeSet::new()).len(),
        1,
        "a single-floor world behaves exactly as a single-node world"
    );
}

// ---------------------------------------------------------------------------------------------
// P05/P06 — the boundary, and the distinction the editor draws
// ---------------------------------------------------------------------------------------------

#[test]
fn an_edge_knows_whether_it_stays_inside_a_space() {
    // ⚠ The editor draws `───` within a Space and `═══` across. The two read as different acts to a
    // designer — climbing a tower is not walking to the next room — even though the solver runs the
    // same machinery on both.
    let (g, _, per) = world(&[2, 1]);
    let mut m = MissionGraph::new(per[0][0]);
    m.add_edge(MissionEdge::open(per[0][0], per[0][1]));
    m.add_edge(MissionEdge::open(per[0][1], per[1][0]));

    let spans: Vec<_> = m
        .edges()
        .iter()
        .map(|e| m.span_of(&g, e).expect("both ends sit in a Space"))
        .collect();
    assert_eq!(
        spans,
        vec![EdgeSpan::WithinSpace, EdgeSpan::CrossesSpace],
        "the first climbs a tower, the second leaves it"
    );
}

#[test]
fn instance_queries_deliberately_stop_at_space_and_not_at_floor() {
    // ⚠ **There is no `InstanceScope::FLOOR`, and its absence is the design.** A floor-scoped instance
    // query would stop at a boundary the geometry does not stop at: a torch on the balcony is still
    // lighting the room below it, and a query that pretended otherwise would answer wrongly in a way
    // no test of the solver would catch.
    //
    // The split: accessibility, spheres and gates are **Floor**-scoped; raycast, overlap,
    // line-of-sight and shape-cast are **Space and up**.
    let manifest = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../manifest/tier1.toml"
    ))
    .expect("the manifest is readable");
    let block = manifest
        .split("path = \"/Core/InstanceScope\"")
        .nth(1)
        .expect("InstanceScope is declared");
    let block = block.split("[[class]]").next().unwrap_or(block);
    assert!(
        !block.contains("name = \"FLOOR\""),
        "InstanceScope must not gain a FLOOR member"
    );
    for expected in ["SPATIAL", "SPACE", "AREA", "REACH", "WORLD"] {
        assert!(
            block.contains(&format!("name = \"{expected}\"")),
            "InstanceScope lost {expected}"
        );
    }
}
