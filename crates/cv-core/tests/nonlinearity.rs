//! Long-range non-linearity: can a gate early in the world have its key *arbitrarily* far away,
//! with most of the world still unrealized?
//!
//! This is the Metroid-Prime-shaped question, and it is worth pinning down because the lifecycle
//! invariant (a node's state may never exceed its parent's) *looks* like it might force the world to
//! be resolved a node at a time. It does not, and the distinction matters:
//!
//! * **Projection is unbounded.** The whole world skeleton — every Reach, Area and Space — can exist
//!   as `Projected` nodes before anything is committed. Nothing caps how far ahead, how deep, or how
//!   broad a projection reaches.
//! * **Projection needs no geometry.** An envelope is only required at `Reserved`, so the entire
//!   logical structure exists before a single dimension is chosen. That is what lets L2 solve
//!   lock-and-key placement over the whole world while L3/L4 have not run at all.
//! * **Realization is selective, not sequential.** Realizing a Space commits its *ancestor chain*, not
//!   its siblings or the Reaches beside it. Reach 5 can be fully built while Reaches 1–4 remain
//!   forecasts.
//! * **References ignore state entirely.** A handle to a `Projected` node is as good as one to a
//!   `Realized` node, so a gate can point at a key that is not merely unplaced but not yet *dimensioned*.
//!
//! Together those mean the solver is free to put a key anywhere in the projected world — the next room
//! or six Reaches away — which is precisely the freedom MP1-style progression needs.

use cv_core::{Handle, Node, NodeGraph, NodeKind, NodeState, Object};
use cv_determinism::{Aabb, Vec3};

/// Stands in for the mission-graph edge M09 will build on top of this structure: a lock in one Space
/// whose key sits in another, with no containment relationship between them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LockAndKey {
    lock_at: Handle<Node>,
    key_at: Handle<Node>,
}

/// Six Reaches × one Area × three Spaces, **entirely projected and entirely dimensionless** — the
/// state of the world when L2 runs, long before any geometry exists.
fn project_whole_world() -> (NodeGraph, Vec<Vec<Handle<Node>>>) {
    let mut g = NodeGraph::new(1.0, 0x00A1_0000_0001);
    let world = g.root();
    let mut spaces_by_reach = Vec::new();

    for r in 0..6 {
        let reach = g.add_child(world, format!("reach_{r}")).unwrap();
        let area = g.add_child(reach, format!("area_{r}")).unwrap();
        let spaces: Vec<Handle<Node>> = (0..3)
            .map(|s| g.add_child(area, format!("space_{r}_{s}")).unwrap())
            .collect();
        for w in spaces.windows(2) {
            g.connect(w[0], w[1]).unwrap();
        }
        spaces_by_reach.push(spaces);
    }
    (g, spaces_by_reach)
}

#[test]
fn the_entire_world_projects_before_anything_is_realized() {
    let (g, spaces) = project_whole_world();
    // 1 World + 6 Reaches + 6 Areas + 18 Spaces.
    assert_eq!(g.len(), 31);
    assert_eq!(g.find(|n| n.state() == NodeState::Projected).count(), 31);
    assert_eq!(spaces.len(), 6);
    assert!(g.check_invariants().is_none());
}

#[test]
fn projection_requires_no_geometry_at_all() {
    // Logic before geometry: L2 solves over this structure while every envelope is still unknown.
    let (g, _) = project_whole_world();
    assert!(
        g.iter().all(|(_, n)| n.envelope().is_none()),
        "a projected world should carry no dimensions yet"
    );
}

#[test]
fn a_gate_can_reference_a_key_five_reaches_away() {
    let (g, spaces) = project_whole_world();

    // The lock is in the very first room; the key is in the last room of the last Reach.
    let relation = LockAndKey {
        lock_at: spaces[0][0],
        key_at: spaces[5][2],
    };

    // Nothing about the reference cares that both ends are unrealized and undimensioned.
    assert_eq!(
        g.node(relation.lock_at).unwrap().state(),
        NodeState::Projected
    );
    assert_eq!(
        g.node(relation.key_at).unwrap().state(),
        NodeState::Projected
    );

    // The two are in genuinely different branches — no containment relationship whatsoever.
    let lock_reach = g.scope_of(relation.lock_at, NodeKind::Reach).unwrap();
    let key_reach = g.scope_of(relation.key_at, NodeKind::Reach).unwrap();
    assert_ne!(lock_reach, key_reach);
    assert_eq!(g.node(lock_reach).unwrap().name(), "reach_0");
    assert_eq!(g.node(key_reach).unwrap().name(), "reach_5");

    // Their only common ancestor is the World itself.
    assert_eq!(
        g.scope_of(relation.lock_at, NodeKind::World),
        Some(g.root())
    );
    assert_eq!(g.scope_of(relation.key_at, NodeKind::World), Some(g.root()));
}

#[test]
fn realizing_the_opening_area_leaves_the_rest_of_the_world_a_forecast() {
    let (mut g, spaces) = project_whole_world();
    let opening = spaces[0][0];

    // Commit only what the opening room needs: its own ancestor chain.
    for h in std::iter::once(opening).chain(g.ancestors_of(opening)) {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
            .unwrap();
    }
    g.advance_with_ancestors(opening, NodeState::Realized)
        .unwrap();

    assert_eq!(g.node(opening).unwrap().state(), NodeState::Realized);
    // Its siblings in the same Area are untouched...
    assert_eq!(g.node(spaces[0][1]).unwrap().state(), NodeState::Projected);
    // ...and so is every later Reach, including the one holding the key.
    for later_reach in spaces.iter().skip(1) {
        for s in later_reach {
            assert_eq!(g.node(*s).unwrap().state(), NodeState::Projected);
        }
    }
    // Exactly four nodes were committed: World → Reach → Area → Space.
    assert_eq!(g.find(|n| n.state() == NodeState::Realized).count(), 4);
    assert!(g.check_invariants().is_none());
}

#[test]
fn the_distant_key_room_can_be_realized_later_without_touching_the_reaches_between() {
    let (mut g, spaces) = project_whole_world();
    let opening = spaces[0][0];
    let key_room = spaces[5][2];

    let boxed = Aabb::new(Vec3::ZERO, Vec3::splat(10.0));
    for h in std::iter::once(opening).chain(g.ancestors_of(opening)) {
        g.set_envelope(h, boxed).unwrap();
    }
    g.advance_with_ancestors(opening, NodeState::Realized)
        .unwrap();

    // Much later — the host explores that far — the key's room is realized on demand.
    //
    // Note the shape of this: only the *undimensioned* part of the chain is given an envelope. The
    // World was committed when the opening was built and is frozen, so realizing a distant region
    // reuses that commitment rather than redoing it. That is the point of the shared ancestor chain.
    for h in std::iter::once(key_room).chain(g.ancestors_of(key_room)) {
        if g.node(h).unwrap().envelope().is_none() {
            g.set_envelope(h, boxed).unwrap();
        }
    }
    g.advance_with_ancestors(key_room, NodeState::Realized)
        .unwrap();

    assert_eq!(g.node(key_room).unwrap().state(), NodeState::Realized);
    // Reaches 1–4 were never involved: realization commits an ancestor *chain*, not a sequence.
    for (r, between) in spaces.iter().enumerate().take(5).skip(1) {
        let reach = g.scope_of(between[0], NodeKind::Reach).unwrap();
        assert_eq!(
            g.node(reach).unwrap().state(),
            NodeState::Projected,
            "reach_{r} should still be a forecast"
        );
    }
    // World + reach_0 chain (3) + reach_5 chain (3) = 7 realized nodes out of 31.
    assert_eq!(g.find(|n| n.state() == NodeState::Realized).count(), 7);
    assert!(g.check_invariants().is_none());
}

#[test]
fn a_projected_region_can_still_be_revised_after_neighbours_are_built() {
    // Forecasts stay revisable while the built part of the world stands — the solver can reroute a
    // late Reach long after the opening is committed.
    let (mut g, spaces) = project_whole_world();
    let opening = spaces[0][0];
    let boxed = Aabb::new(Vec3::ZERO, Vec3::splat(10.0));
    for h in std::iter::once(opening).chain(g.ancestors_of(opening)) {
        g.set_envelope(h, boxed).unwrap();
    }
    g.advance_with_ancestors(opening, NodeState::Realized)
        .unwrap();

    // Discard a whole projected Reach and replace it with a different shape.
    let doomed = g.scope_of(spaces[3][0], NodeKind::Reach).unwrap();
    assert!(
        g.remove(doomed).is_ok(),
        "a forecast may be revised away at any time"
    );

    let replacement = g.add_child(g.root(), "reach_rerouted").unwrap();
    let area = g.add_child(replacement, "area_new").unwrap();
    let new_space = g.add_child(area, "space_new").unwrap();
    assert_eq!(g.node(new_space).unwrap().kind(), NodeKind::Space);

    // The realized opening is untouched by any of it.
    assert_eq!(g.node(opening).unwrap().state(), NodeState::Realized);
    assert!(g.check_invariants().is_none());
}

#[test]
fn depth_of_projection_is_unbounded_in_both_directions() {
    // Breadth and depth are both free while projecting: many Reaches, each fully deep.
    let mut g = NodeGraph::new(1.0, 7);
    let world = g.root();
    for r in 0..50 {
        let reach = g.add_child(world, format!("r{r}")).unwrap();
        let area = g.add_child(reach, "a").unwrap();
        let space = g.add_child(area, "s").unwrap();
        let floor = g.add_child(space, "f").unwrap();
        let spatial = g.add_child(floor, "sp").unwrap();
        assert_eq!(g.depth_of(spatial), 5);
    }
    assert_eq!(g.of_kind(NodeKind::Reach).count(), 50);
    assert_eq!(g.of_kind(NodeKind::Floor).count(), 50);
    assert_eq!(g.of_kind(NodeKind::Spatial).count(), 50);
    assert!(g.check_invariants().is_none());
}
