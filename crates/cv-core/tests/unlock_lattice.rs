//! The unlock lattice, end to end.
//!
//! `unlock.rs` proves the closure in isolation. This proves the thing that actually matters: that
//! expanding a grant through `supersedes` makes the **sweep** open a lock written for a lesser
//! unlock, without `Rule`, `MissionGraph` or the solver knowing `supersedes` exists.
//!
//! ⚠ That indirection is the design. The closure is applied where grants are *collected*, so
//! `Rule::Has(c) => held.contains(c)` and every caller of it stay untouched. Threading a closure
//! through rule evaluation instead would ripple into the solver, the softlock pass and every test
//! fixture, for an identical answer.

use cv_core::unlock::{GrantMap, Unlock, UnlockTable};
use cv_core::{
    Location, LocationId, MissionEdge, MissionGraph, NodeGraph, NodeState, ObjectId, Rule,
};
use cv_determinism::{Aabb, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// `PullToAnchor` ← `LongPullToAnchor`: the Longshot answers a Hookshot lock.
fn ropes() -> UnlockTable {
    UnlockTable::build(vec![
        Unlock::new("u_pull", "PullToAnchor"),
        Unlock::new("u_long", "LongPullToAnchor").superseding("u_pull"),
    ])
    .expect("a valid table")
}

/// start → vault → goal, where the goal is gated on the *lesser* unlock.
fn world(table: &UnlockTable) -> (MissionGraph, BTreeMap<LocationId, ObjectId>) {
    let mut g = NodeGraph::new(1.0, 1);
    let rooms: Vec<_> = (0..3)
        .map(|i| g.add_child(g.root(), format!("room_{i}")).unwrap())
        .collect();
    for h in g.walk() {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
            .unwrap();
        g.advance(h, NodeState::Realized).unwrap();
    }

    let pull = table.by_id("u_pull").expect("u_pull").key();
    let mut m = MissionGraph::new(rooms[0]);
    m.add_edge(MissionEdge::open(rooms[0], rooms[1]));
    // ⚠ The gate names the *lesser* unlock, and never learns about the greater one.
    m.add_edge(MissionEdge::gated(rooms[1], rooms[2], Rule::has(pull)));
    m.set_goal(rooms[2]);
    m.add_location(
        LocationId(0),
        Location {
            scope: rooms[1],
            slot: 0,
        },
    );

    let placements: BTreeMap<LocationId, ObjectId> =
        [(LocationId(0), ObjectId::derived("item", "longshot"))]
            .into_iter()
            .collect();
    (m, placements)
}

#[test]
fn a_superseding_unlock_opens_a_lock_written_for_the_lesser_one() {
    let table = ropes();
    let (m, placements) = world(&table);
    let longshot = ObjectId::derived("item", "longshot");

    // The Longshot grants only `u_long` — expanded through the closure at grant time.
    let grants: GrantMap = [(longshot, table.expand(["u_long"]))].into_iter().collect();

    let swept = m.sweep(&BTreeSet::new(), &placements, &grants);
    assert_eq!(
        swept.depth(),
        2,
        "one sphere to reach the Longshot, one to pass the gate it was never told about"
    );
    assert!(
        swept.accessible(m.goal().expect("a goal")),
        "the goal is gated on PullToAnchor and the player holds only LongPullToAnchor"
    );
}

#[test]
fn without_the_supersedes_edge_the_same_world_is_unsolvable() {
    // The control. If this passed too, the test above would be proving nothing.
    let flat = UnlockTable::build(vec![
        Unlock::new("u_pull", "PullToAnchor"),
        Unlock::new("u_long", "LongPullToAnchor"),
    ])
    .expect("a valid table");
    let (m, placements) = world(&flat);
    let longshot = ObjectId::derived("item", "longshot");
    let grants: GrantMap = [(longshot, flat.expand(["u_long"]))].into_iter().collect();

    let swept = m.sweep(&BTreeSet::new(), &placements, &grants);
    assert!(
        !swept.accessible(m.goal().expect("a goal")),
        "with no ordering declared, a Longshot must not open a Hookshot lock"
    );
}

#[test]
fn one_pickup_can_open_two_unrelated_locks() {
    // Super Metroid's Speed Booster: `run` and `shinespark` gate different rooms, and before M03a
    // the map was `item -> ONE unlock`, so this world was not representable at all.
    let table = UnlockTable::build(vec![
        Unlock::new("u_run", "SustainedSpeed"),
        Unlock::new("u_spark", "Shinespark"),
    ])
    .expect("a valid table");

    let mut g = NodeGraph::new(1.0, 1);
    let rooms: Vec<_> = (0..4)
        .map(|i| g.add_child(g.root(), format!("room_{i}")).unwrap())
        .collect();
    for h in g.walk() {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
            .unwrap();
        g.advance(h, NodeState::Realized).unwrap();
    }

    let run = table.by_id("u_run").expect("u_run").key();
    let spark = table.by_id("u_spark").expect("u_spark").key();
    let mut m = MissionGraph::new(rooms[0]);
    m.add_edge(MissionEdge::open(rooms[0], rooms[1]));
    m.add_edge(MissionEdge::gated(rooms[1], rooms[2], Rule::has(run)));
    m.add_edge(MissionEdge::gated(rooms[1], rooms[3], Rule::has(spark)));
    m.set_goal(rooms[3]);
    m.add_location(
        LocationId(0),
        Location {
            scope: rooms[1],
            slot: 0,
        },
    );

    let booster = ObjectId::derived("item", "speed_booster");
    let placements: BTreeMap<LocationId, ObjectId> =
        [(LocationId(0), booster)].into_iter().collect();
    let grants: GrantMap = [(booster, table.expand(["u_run", "u_spark"]))]
        .into_iter()
        .collect();

    let swept = m.sweep(&BTreeSet::new(), &placements, &grants);
    assert!(swept.accessible(rooms[2]), "the run gate opened");
    assert!(swept.accessible(rooms[3]), "the shinespark gate opened too");
}
