//! M11 exit criteria: the spatial primitives mechanics reason through.
//!
//! The load-bearing claim is not "raycast works" — it is that **geometry answers *where*, and mechanics
//! answer *whether***. If the two ever merge, `FlowKind` leaks into the primitives, and a laser puzzle
//! stops being expressible without the core knowing what a laser is.
//!
//! So the centrepiece here is a laser tracer built entirely out of `raycast_all` plus the fixture
//! mechanics' own `blocks`/`redirects`. It is roughly forty lines, it lives in the *test*, and the
//! geometry module knows nothing about it — which is the whole point.

use cv_core::fixtures::{Deflective, Glass};
use cv_core::{
    CoarseGeometry, Collider, ContentKind, ContentRegistry, Context, Face, FlowKind, Hit, Mechanic,
    MechanicRegistry, NodeGraph, NodeKind, NodeState, ObjectId,
};
use cv_determinism::{Aabb, Rng, Vec3};

fn oid(name: &str) -> ObjectId {
    ObjectId::derived("actor", name)
}

/// A plain wall: stops everything, deflects nothing.
///
/// Needed because `Mechanic::blocks` defaults to **false** — a thing that says nothing about blocking
/// does not block. That default is right (unregistered content should not silently become solid), but
/// it does mean "an obstacle" is something a test has to state rather than assume.
struct Solid;

impl Mechanic for Solid {
    fn kind(&self) -> ContentKind {
        ContentKind::Actor
    }
    fn blocks(&self, _ctx: &Context<'_>, _flow: FlowKind) -> bool {
        true
    }
}

fn box_at(min: Vec3, max: Vec3) -> Aabb {
    Aabb::new(min, max)
}

/// A world with no scopes worth speaking of — the primitives do not need one.
fn bare_world() -> (
    NodeGraph,
    ContentRegistry,
    Vec<(cv_core::Handle<cv_core::Node>, ObjectId)>,
) {
    let mut g = NodeGraph::new(1.0, 1);
    let reach = g.add_child(g.root(), "reach").unwrap();
    let area = g.add_child(reach, "area").unwrap();
    let space = g.add_child(area, "space").unwrap();
    for h in [g.root(), reach, area, space] {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(64.0)))
            .unwrap();
        g.advance(h, NodeState::Realized).unwrap();
    }
    (g, ContentRegistry::new(), Vec::new())
}

// ---------------------------------------------------------------------------------------------
// The laser tracer — built from primitives + mechanics, knowing nothing about either's internals
// ---------------------------------------------------------------------------------------------

/// One leg of a traced beam.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Leg {
    from: Vec3,
    to: Vec3,
}

/// Trace a beam of `flow` until something blocks it, bouncing off whatever redirects it.
///
/// Deliberately generic over the flow: the same routine traces a laser, a line of sight, or a bullet,
/// and the *only* thing that differs is what the mechanics say. That is the property under test.
fn trace(
    ctx: &Context<'_>,
    mechanics: &MechanicRegistry,
    flow: FlowKind,
    mut origin: Vec3,
    mut direction: Vec3,
    range: f64,
    max_bounces: usize,
) -> Vec<Leg> {
    let mut legs = Vec::new();
    let mut budget = range;

    for _ in 0..=max_bounces {
        let hits: Vec<Hit> = ctx.raycast_all(origin, direction, budget);
        // The first surface that actually stops *this* flow — everything nearer is passed through.
        let stopper = hits
            .iter()
            .find(|h| !h.from_inside && mechanics.get(h.owner).blocks(ctx, flow));
        let redirector = hits.iter().find(|h| {
            !h.from_inside
                && mechanics
                    .get(h.owner)
                    .redirects(ctx, flow, direction)
                    .is_some()
        });

        // Whichever comes first wins; a mirror that also blocked would simply stop the beam.
        let event = match (stopper, redirector) {
            (Some(s), Some(r)) if s.distance <= r.distance => Some((*s, true)),
            (_, Some(r)) => Some((*r, false)),
            (Some(s), None) => Some((*s, true)),
            (None, None) => None,
        };

        let Some((hit, blocked)) = event else {
            legs.push(Leg {
                from: origin,
                to: origin + direction.normalized() * budget,
            });
            return legs;
        };

        legs.push(Leg {
            from: origin,
            to: hit.point,
        });
        if blocked {
            return legs;
        }
        let Some(bounced) = mechanics.get(hit.owner).redirects(ctx, flow, direction) else {
            return legs;
        };
        budget -= hit.distance;
        if budget <= 0.0 {
            return legs;
        }
        // Step off the surface so the next cast does not immediately re-hit it.
        origin = hit.point + bounced.normalized() * 1e-6;
        direction = bounced;
    }
    legs
}

#[test]
fn a_laser_passes_glass_and_stops_at_stone_while_a_bullet_stops_at_the_glass() {
    // The TC16 case, and the reason `blocks` takes a `FlowKind` while `raycast` does not: one ray, one
    // set of hits, two different answers depending on what is travelling.
    let (g, reg, placed) = bare_world();
    let glass = oid("glass");
    let stone = oid("stone");

    let mut geometry = CoarseGeometry::new();
    geometry.add(Collider::new(
        glass,
        box_at(Vec3::new(2.0, 0.0, 0.0), Vec3::new(2.5, 4.0, 4.0)),
    ));
    geometry.add(Collider::new(
        stone,
        box_at(Vec3::new(6.0, 0.0, 0.0), Vec3::new(7.0, 4.0, 4.0)),
    ));

    let mut mechanics = MechanicRegistry::new();
    mechanics.register(glass, Box::new(Glass));
    mechanics.register(stone, Box::new(Solid));

    let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "laser").with_geometry(&geometry);
    let start = Vec3::new(0.0, 2.0, 2.0);

    let laser = trace(&ctx, &mechanics, FlowKind::Laser, start, Vec3::X, 20.0, 0);
    assert_eq!(laser.len(), 1);
    assert_eq!(
        laser[0].to.x, 6.0,
        "the laser passed the glass and stopped at stone"
    );

    let bullet = trace(
        &ctx,
        &mechanics,
        FlowKind::Ballistic,
        start,
        Vec3::X,
        20.0,
        0,
    );
    assert_eq!(bullet.len(), 1);
    assert_eq!(bullet[0].to.x, 2.0, "the bullet stopped at the glass");

    // Sight behaves like the laser; walking like the bullet. Same geometry, no geometry changes.
    let sight = trace(&ctx, &mechanics, FlowKind::Sight, start, Vec3::X, 20.0, 0);
    assert_eq!(sight[0].to.x, 6.0);
    let walk = trace(&ctx, &mechanics, FlowKind::Walking, start, Vec3::X, 20.0, 0);
    assert_eq!(walk[0].to.x, 2.0);
}

#[test]
fn a_laser_bounces_off_a_mirror_into_a_catcher() {
    // The puzzle shape the design doc names: a beam that only reaches its target via a reflection.
    let (g, reg, placed) = bare_world();
    let mirror = oid("mirror");
    let catcher = oid("catcher");

    let mut geometry = CoarseGeometry::new();
    // A mirror at x≈10 facing -X, so a +X beam comes back along -X... instead angle it: the mirror's
    // normal is -X, and the beam arrives along +X, so it reflects straight back.
    geometry.add(Collider::new(
        mirror,
        box_at(Vec3::new(10.0, 0.0, 0.0), Vec3::new(11.0, 4.0, 4.0)),
    ));
    // The catcher sits behind the emitter, reachable only by the bounce.
    geometry.add(Collider::new(
        catcher,
        box_at(Vec3::new(-3.0, 0.0, 0.0), Vec3::new(-2.0, 4.0, 4.0)),
    ));

    let mut mechanics = MechanicRegistry::new();
    mechanics.register(
        mirror,
        Box::new(Deflective::facing(Vec3::new(-1.0, 0.0, 0.0))),
    );
    mechanics.register(catcher, Box::new(Solid));

    let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "laser").with_geometry(&geometry);
    let start = Vec3::new(0.0, 2.0, 2.0);

    let legs = trace(&ctx, &mechanics, FlowKind::Laser, start, Vec3::X, 60.0, 4);
    assert_eq!(legs.len(), 2, "out to the mirror, back to the catcher");
    assert_eq!(legs[0].to.x, 10.0, "reached the mirror");
    assert!(
        (legs[1].to.x - (-2.0)).abs() < 1e-6,
        "the bounce reached the catcher, ending at x = {}",
        legs[1].to.x
    );

    // Swap the mirror for a plain wall and the catcher goes dark — the bounce was doing the work.
    let mut plain = MechanicRegistry::new();
    plain.register(mirror, Box::new(Solid));
    let unbounced = trace(&ctx, &plain, FlowKind::Laser, start, Vec3::X, 60.0, 4);
    assert_eq!(unbounced.len(), 1, "no reflection, no second leg");
    assert_eq!(unbounced[0].to.x, 10.0);

    // And an *unregistered* surface does not block at all: the beam sails past everything.
    let none = MechanicRegistry::new();
    let unobstructed = trace(&ctx, &none, FlowKind::Laser, start, Vec3::X, 60.0, 4);
    assert_eq!(unobstructed.len(), 1);
    assert_eq!(
        unobstructed[0].to.x, 60.0,
        "blocking is opt-in; geometry alone stops nothing"
    );
}

#[test]
fn a_bullet_does_not_bounce_off_a_mirror() {
    // `redirects` is per-flow too: the same mirror reflects a laser and simply stops a bullet.
    let (g, reg, placed) = bare_world();
    let mirror = oid("mirror");
    let mut geometry = CoarseGeometry::new();
    geometry.add(Collider::new(
        mirror,
        box_at(Vec3::new(10.0, 0.0, 0.0), Vec3::new(11.0, 4.0, 4.0)),
    ));
    let mut mechanics = MechanicRegistry::new();
    mechanics.register(
        mirror,
        Box::new(Deflective::facing(Vec3::new(-1.0, 0.0, 0.0))),
    );

    let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "shot").with_geometry(&geometry);
    let legs = trace(
        &ctx,
        &mechanics,
        FlowKind::Ballistic,
        Vec3::new(0.0, 2.0, 2.0),
        Vec3::X,
        60.0,
        4,
    );
    assert_eq!(
        legs.len(),
        1,
        "a bullet stops where a laser would have turned"
    );
    assert_eq!(legs[0].to.x, 10.0);
}

// ---------------------------------------------------------------------------------------------
// Movement
// ---------------------------------------------------------------------------------------------

#[test]
fn sweeping_and_sliding_move_a_body_through_coarse_geometry() {
    let (g, reg, placed) = bare_world();
    let mut geometry = CoarseGeometry::new();
    geometry.add(Collider::new(
        oid("wall"),
        box_at(Vec3::new(4.0, -8.0, -8.0), Vec3::new(5.0, 8.0, 8.0)),
    ));
    let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "move").with_geometry(&geometry);

    let body = Aabb::from_center_extents(Vec3::ZERO, Vec3::splat(0.5));
    let head_on = ctx.sweep(body, Vec3::X, 20.0);
    assert_eq!(head_on.distance, 3.5, "stopped with its face on the wall");
    assert_eq!(head_on.hit.unwrap().face, Face::NegX);

    // At a glancing angle it should carry along the wall rather than stopping dead.
    let glancing = ctx.slide_to_collision(body, Vec3::new(1.0, 0.0, 1.0), 20.0);
    assert!(glancing.distance > head_on.distance);
    assert!(glancing.end.x <= 3.5 + 1e-6, "did not tunnel through");
    assert!(glancing.end.z > 3.0, "and made progress along the face");
}

// ---------------------------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------------------------

#[test]
fn primitives_are_reproducible_and_order_independent() {
    let (g, reg, placed) = bare_world();
    let mut geometry = CoarseGeometry::new();
    for i in 0..24 {
        let x = f64::from(i) * 1.7 - 3.3;
        geometry.add(Collider::new(
            oid(&format!("box_{i}")),
            box_at(Vec3::new(x, -0.25, -0.25), Vec3::new(x + 0.9, 1.25, 1.25)),
        ));
    }
    let ctx = Context::new(&g, &reg, &placed, &Rng::new(1), "d").with_geometry(&geometry);

    let origin = Vec3::new(-10.0, 0.5, 0.5);
    let first = ctx.raycast_all(origin, Vec3::new(1.0, 0.0, 0.0), 100.0);
    assert!(first.len() > 4, "the fixture must actually hit things");
    for _ in 0..16 {
        assert_eq!(
            ctx.raycast_all(origin, Vec3::new(1.0, 0.0, 0.0), 100.0),
            first
        );
    }
    // Distances are non-decreasing, which is what makes the flow-selective march correct.
    for pair in first.windows(2) {
        assert!(pair[0].distance <= pair[1].distance);
    }

    // A direction given un-normalized must answer identically to the normalized one.
    let scaled = ctx.raycast_all(origin, Vec3::new(7.0, 0.0, 0.0), 100.0);
    assert_eq!(
        scaled, first,
        "magnitude is range, not part of the direction"
    );
}

#[test]
fn geometry_built_from_scopes_matches_the_realized_world() {
    let mut g = NodeGraph::new(1.0, 3);
    let reach = g.add_child(g.root(), "reach").unwrap();
    let area = g.add_child(reach, "area").unwrap();
    let rooms: Vec<_> = (0..4)
        .map(|i| g.add_child(area, format!("room_{i}")).unwrap())
        .collect();
    for (i, h) in rooms.iter().enumerate() {
        let x = i as f64 * 10.0;
        g.set_envelope(
            *h,
            Aabb::new(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 8.0, 4.0, 8.0)),
        )
        .unwrap();
    }
    for h in [g.root(), reach, area] {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(64.0)))
            .unwrap();
        g.advance(h, NodeState::Realized).unwrap();
    }
    for h in &rooms[..3] {
        g.advance(*h, NodeState::Realized).unwrap();
    }

    let geometry = CoarseGeometry::from_scopes(&g, NodeKind::Space);
    assert_eq!(geometry.len(), 3, "the fourth room is still a forecast");

    // A ray down the row meets the realized rooms in order and never the projected one.
    let hits = geometry.raycast_all(Vec3::new(-5.0, 2.0, 4.0), Vec3::X, 100.0);
    let owners: Vec<ObjectId> = hits.iter().map(|h| h.owner).collect();
    let expected: Vec<ObjectId> = rooms[..3]
        .iter()
        .map(|h| {
            use cv_core::Object;
            g.get(*h).unwrap().id()
        })
        .collect();
    assert_eq!(owners, expected);
}
