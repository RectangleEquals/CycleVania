//! M07a exit criteria: **a bonfire composes from two core components with no special case.**
//!
//! > **Green when:** a bonfire — one Actor carrying **both** a `CheckpointComponent` and a
//! > `FastTravelComponent` — composes with no special case, and a bench and a stag each carry one.
//!
//! The criterion looks trivial and is not. It is the difference between a component model and a
//! taxonomy: if a bonfire needed core support, then so would every future pairing, and the eight
//! components would be a list of *nouns the engine knows about* rather than *behaviours content
//! composes*.
//!
//! ⚠ **The second half of the criterion carries as much weight as the first.** A bonfire that works
//! only because something special-cased *bonfire* would still pass the first half. It passes the
//! second half only if the two halves work alone — which is what makes the whole thing a composition.

use cv_core::collision::{CollisionBody, CollisionData, CollisionLayer};
use cv_core::component::{Attached, CollisionMode, Component, Components, Direction};
use cv_core::judge::Budget;
use cv_core::mission::Rule;
use cv_core::schedule::Span;
use cv_core::shape::Shape;
use cv_core::surface::Occupant;
use cv_core::tag::TagQuery;
use cv_core::{InstanceScope, ObjectId};
use cv_determinism::Vec3;

fn oid(name: &str) -> ObjectId {
    ObjectId::derived("actor", name)
}
fn unlock(name: &str) -> ObjectId {
    ObjectId::derived("unlock", name)
}

/// The behaviour, not the noun: *the world can be restored from here*.
fn checkpoint(scope: InstanceScope) -> Component {
    Component::Checkpoint {
        restores: vec![oid("enemies"), oid("breakables")],
        restores_occupant: true,
        scope,
    }
}

/// The behaviour, not the noun: *this is a node in a network*.
fn fast_travel(network: &str, unlocked_by: Rule) -> Component {
    Component::FastTravel {
        network: network.to_string(),
        cost: Some(Budget::distance(0.0)),
        unlocked_by,
    }
}

/// A visible, collidable body so the composed Actor has geometry to aggregate.
fn body(height: f64) -> Component {
    Component::Shape {
        shape: Shape::Cylinder {
            radius_top: 0.8,
            radius_bottom: 1.0,
            height,
            capped: true,
        },
        surface: Some(oid("stone")),
        collision_mode: CollisionMode::Exact,
        visible: true,
    }
}

// ---------------------------------------------------------------------------------------------
// The criterion
// ---------------------------------------------------------------------------------------------

#[test]
fn a_bonfire_a_bench_and_a_stag_are_one_model_and_not_three() {
    // A bonfire is both.
    let bonfire = Components::new()
        .with(Attached::new(body(0.6)))
        .with(Attached::new(checkpoint(InstanceScope::Area)))
        .with(Attached::new(fast_travel("world", Rule::Always)));

    // A bench restores and goes nowhere.
    let bench = Components::new()
        .with(Attached::new(body(0.5)))
        .with(Attached::new(checkpoint(InstanceScope::Space)));

    // A stag goes somewhere and restores nothing.
    let stag = Components::new()
        .with(Attached::new(body(2.2)))
        .with(Attached::new(fast_travel(
            "stagways",
            Rule::has(unlock("stag_pass")),
        )));

    assert!(bonfire.is_checkpoint() && !bonfire.networks().is_empty());
    assert!(bench.is_checkpoint() && bench.networks().is_empty());
    assert!(!stag.is_checkpoint() && !stag.networks().is_empty());

    // ⚠ The composition claim, stated exactly: the bonfire's two behaviours are *the same two* the
    // bench and the stag carry. Nothing about it is a third thing.
    let bonfire_hooks: Vec<&str> = bonfire
        .enabled()
        .map(Component::name)
        .filter(|n| *n != "ShapeComponent")
        .collect();
    let bench_hooks: Vec<&str> = bench
        .enabled()
        .map(Component::name)
        .filter(|n| *n != "ShapeComponent")
        .collect();
    let stag_hooks: Vec<&str> = stag
        .enabled()
        .map(Component::name)
        .filter(|n| *n != "ShapeComponent")
        .collect();
    assert_eq!(bonfire_hooks, [bench_hooks, stag_hooks].concat());
}

#[test]
fn the_two_halves_of_a_bonfire_stay_independent_when_one_is_gated() {
    // A stag's network is locked behind a pass; a bonfire's checkpoint is not. Gating one must not
    // reach the other — which is only automatic if they really are separate components.
    let gated = Components::new()
        .with(Attached::new(checkpoint(InstanceScope::Area)))
        .with(Attached::new(fast_travel(
            "stagways",
            Rule::has(unlock("stag_pass")),
        )));

    let locked = gated
        .enabled()
        .filter_map(|c| match c {
            Component::FastTravel { unlocked_by, .. } => Some(unlocked_by.clone()),
            _ => None,
        })
        .next()
        .expect("a network");
    assert!(!locked.is_open(), "the network is gated");
    assert!(
        gated.is_checkpoint(),
        "and the checkpoint is not, because it never asked"
    );
}

// ---------------------------------------------------------------------------------------------
// Aggregation is a default, not an absence
// ---------------------------------------------------------------------------------------------

#[test]
fn attaching_a_component_changes_the_answer_without_anyone_forwarding_it() {
    // ⚠ **The failure the aggregation default exists to prevent.** If each hook had to be forwarded by
    // hand, one missing line would make a component *do nothing* — no error, no warning. Here,
    // attaching geometry moves the collision answer on its own.
    let bare = Components::new().with(Attached::new(checkpoint(InstanceScope::Space)));
    assert!(
        bare.collision().is_empty(),
        "a checkpoint has no geometry of its own"
    );

    let with_body = bare.clone().with(Attached::new(body(0.6)));
    assert_eq!(with_body.collision().len(), 1);
    assert!(!with_body.bounds().is_empty());
}

#[test]
fn the_four_spatial_questions_stay_four_questions() {
    // ⚠ An elevator reserves a shaft *and* requires a landing at each end. Fusing footprint and
    // clearance means a developer inflates the footprint to fake the clearance, and worlds go sparse.
    let elevator = Components::new()
        .with(Attached::new(body(3.0)))
        .with(Attached::new(Component::traversal(
            Span::exactly(0.0),
            Span::new(0.0, 12.0),
            Rule::Always,
            Rule::Always,
        )));

    let footprint = elevator.footprint();
    let clearance = elevator.clearance();
    assert!(!footprint.is_empty(), "the car occupies the shaft");
    assert!(
        !clearance.is_empty(),
        "the ride needs room it does not occupy"
    );
    assert_ne!(
        footprint.bounds(),
        clearance.bounds(),
        "reserved space and required-empty space are not the same volume"
    );
}

#[test]
fn clearance_is_empty_by_default_rather_than_the_collision_union() {
    // ⚠ Defaulting clearance to collision would reserve a hole the size of every object around every
    // object. Most content needs no free space at all.
    let plain = Components::new().with(Attached::new(body(1.0)));
    assert!(!plain.collision().is_empty());
    assert!(plain.clearance().is_empty());
}

#[test]
fn disabling_a_component_removes_it_from_every_hook_at_once() {
    // One toggle, all hooks — the other half of the aggregation default. A per-hook enable check would
    // eventually miss one.
    let mut all = Components::new()
        .with(Attached::new(body(1.0)))
        .with(Attached::new(checkpoint(InstanceScope::Space)))
        .with(Attached::new(fast_travel("world", Rule::Always)));

    assert_eq!(all.collision().len(), 1);
    assert!(all.is_checkpoint() && !all.networks().is_empty());

    all = Components::new()
        .with(Attached::new(body(1.0)).disabled())
        .with(Attached::new(checkpoint(InstanceScope::Space)).disabled())
        .with(Attached::new(fast_travel("world", Rule::Always)));

    assert!(all.collision().is_empty());
    assert!(!all.is_checkpoint());
    assert_eq!(
        all.networks(),
        vec!["world"],
        "the enabled one still answers"
    );
    assert_eq!(all.all().len(), 3, "and nothing was removed");
}

#[test]
fn the_lint_number_is_readable_from_the_component_set() {
    // ⚠ The editor's *"you forgot a forwarding line"* lint compares an override's coverage against
    // this. It is only checkable because the aggregate knows how many components answer each hook.
    let hub = Components::new()
        .with(Attached::new(fast_travel("regional", Rule::Always)))
        .with(Attached::new(fast_travel("world", Rule::Always)))
        .with(Attached::new(checkpoint(InstanceScope::Reach)));
    assert_eq!(hub.contributors("FastTravelComponent"), 2);
    assert_eq!(hub.contributors("CheckpointComponent"), 1);
    assert_eq!(hub.contributors("TraversalComponent"), 0);
}

// ---------------------------------------------------------------------------------------------
// The parts of the set that are load-bearing elsewhere
// ---------------------------------------------------------------------------------------------

#[test]
fn a_checkpoint_is_scoped_and_the_scope_is_a_real_choice() {
    // ⚠ *"Restore this room"* and *"restore this region"* are different promises. P15's second
    // satisfaction route is only usable if the solver knows how much comes back.
    let room = checkpoint(InstanceScope::Space);
    let region = checkpoint(InstanceScope::Reach);
    let (Component::Checkpoint { scope: a, .. }, Component::Checkpoint { scope: b, .. }) =
        (&room, &region)
    else {
        panic!("checkpoints");
    };
    assert_ne!(a, b);
    assert!(a.node_kind().depth() > b.node_kind().depth());
}

#[test]
fn a_gate_that_names_a_nearby_kind_must_say_at_what_scope() {
    // ⚠ *"A Bomb Flower within carry range"* asked at `Space` and at `World` are different questions
    // with different answers. A rule that could not say which would silently answer the wrong one.
    let near = Rule::Nearby {
        kind: oid("bomb_flower"),
        within: 8.0,
        scope: InstanceScope::Space,
    };
    let far = Rule::Nearby {
        kind: oid("bomb_flower"),
        within: 8.0,
        scope: InstanceScope::World,
    };
    assert_ne!(near, far);
    assert!(near.explain().contains("Space"), "{}", near.explain());
    assert!(far.explain().contains("World"), "{}", far.explain());
}

#[test]
fn there_is_no_floor_scoped_instance_query() {
    // ⚠ A floor-scoped instance query would stop at a boundary the geometry does not stop at. `Floor`
    // answers *"can the occupant get there"*; instance queries answer *"what is physically present"*.
    use cv_core::NodeKind;
    assert_eq!(InstanceScope::ALL.len(), 4);
    assert_eq!(InstanceScope::for_kind(NodeKind::Floor), None);
    assert_eq!(InstanceScope::for_kind(NodeKind::Spatial), None);
    assert_eq!(
        InstanceScope::for_kind(NodeKind::Space),
        Some(InstanceScope::Space)
    );
}

#[test]
fn a_traversal_is_directed_and_a_barrier_closes_it_rather_than_deleting_it() {
    // ⚠ P2 — gate a region, never delete it. The barrier names the traversal *kind* it closes, so it
    // is placed *on* an edge instead of replacing the geometry with a wall.
    let key = unlock("vault_key");
    let door = Component::traversal(
        Span::exactly(2.0),
        Span::exactly(0.0),
        Rule::has(key),
        Rule::Always,
    );
    assert!(!door.admits(Direction::Forward, &Occupant::player([])));
    assert!(door.admits(Direction::Reverse, &Occupant::player([])));
    assert!(door.admits(Direction::Forward, &Occupant::player([key])));

    let bar = Component::BlocksTraversal {
        matching: oid("VaultDoor"),
        route: None,
    };
    assert_eq!(bar.name(), "BlocksTraversalComponent");
}

#[test]
fn a_traversals_default_clearance_over_reserves_rather_than_under_reserves() {
    // ⚠ P1 — a jump's real arc is a parabola *inside* the box implied by run × rise, never outside it.
    // A developer who tightens it is making a claim; one who ignores it is safe.
    let leap = Component::traversal(
        Span::new(0.0, 6.0),
        Span::new(0.0, 3.0),
        Rule::Always,
        Rule::Always,
    );
    let Component::Traversal { clearance, .. } = &leap else {
        panic!("a traversal");
    };
    let b = clearance.bounds();
    assert!(b.size().x >= 6.0, "at least the run");
    assert!(b.size().y >= 3.0, "at least the rise");
}

#[test]
fn a_mount_accepts_a_query_rather_than_a_list_that_goes_stale() {
    // ⚠ The filters-instead-of-ids problem. A socket written as *"these four torches"* silently
    // excludes the fifth; written as a query it does not.
    let sconce = Component::Mount {
        name: "sconce".into(),
        accepts: TagQuery::inherited("Prop.Torch"),
        faces: vec![cv_core::Face::PosY],
        clearance: CollisionBody::of(Shape::Cube {
            extents: Vec3::new(0.5, 1.2, 0.5),
            bevel: 0.0,
        }),
    };
    let Component::Mount { accepts, .. } = &sconce else {
        panic!("a mount");
    };
    assert!(accepts.matches(&cv_core::Tag::new("Prop.Torch.Everburning")));
    assert!(!accepts.matches(&cv_core::Tag::new("Prop.Torchbearer")));
}

#[test]
fn the_coarse_layer_and_the_realized_layer_never_get_confused() {
    // ⚠ The L2c hull and L4 geometry both collide. Reporting a conservative answer as a firm one is
    // the failure this separation exists to prevent.
    let mut b = CollisionBody::empty();
    b.add(
        CollisionData::new(Shape::Cube {
            extents: Vec3::new(8.0, 4.0, 8.0),
            bevel: 0.0,
        })
        .on(CollisionLayer::Hull),
    );
    b.add(CollisionData::new(Shape::Sphere { radius: 1.0 }).on(CollisionLayer::Static));
    assert_eq!(
        b.islands()
            .iter()
            .filter(|i| i.layer == CollisionLayer::Hull)
            .count(),
        1
    );
    assert_eq!(
        b.islands()
            .iter()
            .filter(|i| i.layer == CollisionLayer::Static)
            .count(),
        1,
        "and the realized island is still separately visible"
    );
}
