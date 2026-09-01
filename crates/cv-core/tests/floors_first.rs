//! M05 exit criteria: **floors first, and spatial queries live at L2c**.
//!
//! The claim under test is an *ordering* one, and it is what dissolves the chicken-and-egg between
//! content hooks that need spatial answers and geometry that is expensive to build:
//!
//! > **Floors do not depend on hulls; hulls depend on floors.**
//!
//! So the shape here is: build only floor collision, run the ladder, and show that a callback holding
//! nothing but a `Context` gets a real answer — before any hull, mesh or volume exists.

use cv_core::floor::FloorLadder;
use cv_core::{
    CoarseGeometry, Collider, ContentRegistry, Context, NodeGraph, NodeState, ObjectId, Trivalent,
};
use cv_determinism::{Aabb, Rng, Vec3};

fn oid(n: &str) -> ObjectId {
    ObjectId::derived("actor", n)
}

/// A world of two rooms, with nothing in it yet.
fn world() -> (NodeGraph, Vec<cv_core::Handle<cv_core::Node>>) {
    let mut g = NodeGraph::new(1.0, 1);
    let reach = g.add_child(g.root(), "reach").unwrap();
    let area = g.add_child(reach, "area").unwrap();
    let rooms: Vec<_> = (0..2)
        .map(|i| g.add_child(area, format!("room_{i}")).unwrap())
        .collect();
    for h in g.walk() {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(64.0)))
            .unwrap();
        g.advance(h, NodeState::Realized).unwrap();
    }
    (g, rooms)
}

#[test]
fn a_hook_gets_a_real_spatial_answer_before_anything_expensive_exists() {
    let (g, rooms) = world();

    // L2a — floor collision only. No hulls, no meshes, no volumes.
    let mut geo = CoarseGeometry::new();
    geo.add(
        Collider::new(
            oid("ledge"),
            Aabb::new(Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0)),
        )
        .in_scope(rooms[0]),
    );

    // L2b/L2c — the bounds derived from that floor.
    let ladder = FloorLadder::build(&geo, 50.0, 1.9);

    // L2d — a callback, holding only a context.
    let reg = ContentRegistry::new();
    let placed: Vec<(cv_core::Handle<cv_core::Node>, ObjectId)> = Vec::new();
    let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "requires")
        .with_geometry(&geo)
        .with_floors(&ladder);

    let on_the_ledge = Vec3::new(2.0, 1.5, 2.0);
    assert_eq!(
        ctx.standable(rooms[0], on_the_ledge),
        Trivalent::Yes,
        "a hook can ask where an occupant could be, with only floor built"
    );
    assert_eq!(
        ctx.standable(rooms[0], Vec3::new(40.0, 1.5, 2.0)),
        Trivalent::No,
        "and outside the outer bound is a definite no"
    );
}

#[test]
fn before_the_ladder_runs_a_hook_gets_no_answer_rather_than_a_wrong_one() {
    // ⚠ The distinction M06 formalises as `Trivalent`. A hook that runs early must not read "nothing
    // there" from "not computed yet" — that is exactly how an optimistic bound becomes a lie.
    let (g, rooms) = world();
    let reg = ContentRegistry::new();
    let placed: Vec<(cv_core::Handle<cv_core::Node>, ObjectId)> = Vec::new();
    let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "early");

    assert!(ctx.bounds(rooms[0]).is_none(), "no ladder, no answer");
    // ⚠ **The gap M05a left, now closed.** Before `Trivalent` both of these answered `false`, which
    // reads as "nothing there" — a confident wrong answer. `AMBIGUOUS` says what is true: nothing is
    // known yet, so re-ask at the next rung.
    assert_eq!(ctx.standable(rooms[0], Vec3::ZERO), Trivalent::Ambiguous);
}

#[test]
fn the_answer_sharpens_as_floor_is_committed_and_never_reverses() {
    // The monotone claim, exercised across two steps of the ladder rather than asserted about one.
    // Adding floor may turn a "no" into a "yes"; it must never turn a definite "yes" into a "no".
    let (g, rooms) = world();
    let reg = ContentRegistry::new();
    let placed: Vec<(cv_core::Handle<cv_core::Node>, ObjectId)> = Vec::new();
    let probe = Vec3::new(2.0, 1.5, 2.0);

    let mut geo = CoarseGeometry::new();
    geo.add(
        Collider::new(oid("a"), Aabb::new(Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0))).in_scope(rooms[0]),
    );
    let first = FloorLadder::build(&geo, 50.0, 1.9);
    let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "a").with_floors(&first);
    assert_eq!(ctx.standable(rooms[0], probe), Trivalent::Yes);

    // Commit more floor in the same scope.
    geo.add(
        Collider::new(
            oid("b"),
            Aabb::new(Vec3::new(8.0, 0.0, 0.0), Vec3::new(12.0, 1.0, 4.0)),
        )
        .in_scope(rooms[0]),
    );
    let second = FloorLadder::build(&geo, 50.0, 1.9);
    let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "b").with_floors(&second);
    assert_eq!(
        ctx.standable(rooms[0], probe),
        Trivalent::Yes,
        "committing more floor must never retract a definite answer"
    );
}

#[test]
fn a_scope_with_no_floor_is_answerable_not_absent() {
    let (g, rooms) = world();
    let mut geo = CoarseGeometry::new();
    geo.add(
        Collider::new(oid("a"), Aabb::new(Vec3::ZERO, Vec3::new(4.0, 1.0, 4.0))).in_scope(rooms[0]),
    );
    let ladder = FloorLadder::build(&geo, 50.0, 1.9);

    let reg = ContentRegistry::new();
    let placed: Vec<(cv_core::Handle<cv_core::Node>, ObjectId)> = Vec::new();
    let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "t").with_floors(&ladder);

    // ⚠ Room 1 got no floor. That is a fact about the world; "the ladder has not run" is a fact about
    // the computation. Both answer `AMBIGUOUS`, and that is **correct** — neither is a definite
    // "nothing is there", and the honest response to both is to ask again at the next rung.
    assert!(ctx.bounds(rooms[1]).is_none());
    assert_eq!(ctx.standable(rooms[1], Vec3::ZERO), Trivalent::Ambiguous);
}
