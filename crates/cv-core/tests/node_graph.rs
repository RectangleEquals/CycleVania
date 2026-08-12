//! M04 exit criteria: a multi-scope world builds and traverses deterministically, round-trips through
//! serialization intact, and rejects illegal writes to realized nodes.
//!
//! The unit tests in `node.rs` cover each rule in isolation; this file exercises them together on a
//! world shaped like one the pipeline will actually produce — several Reaches at different lifecycle
//! stages, connected Spaces, and a partially-realized subtree.

use cv_core::serialize::{from_bytes, to_bytes, SerError, Writer};
use cv_core::{Handle, Node, NodeError, NodeGraph, NodeKind, NodeState, Object};
use cv_determinism::{Aabb, Vec3};

fn boxed(size: f64) -> Aabb {
    Aabb::new(Vec3::ZERO, Vec3::splat(size))
}

/// A world with two Reaches: the first fully realized, the second still an abstract forecast —
/// exactly the shape lazy generation produces mid-run.
fn build_world() -> (NodeGraph, Handle<Node>, Handle<Node>, Vec<Handle<Node>>) {
    let mut g = NodeGraph::new(1.0, 0xC0FFEE);
    let world = g.root();
    g.set_envelope(world, boxed(1000.0)).unwrap();

    // Reach 1 — built out and realized.
    let near = g.add_child(world, "reach_near").unwrap();
    let area = g.add_child(near, "area_entry").unwrap();
    let spaces: Vec<Handle<Node>> = (0..4)
        .map(|i| g.add_child(area, format!("space_{i}")).unwrap())
        .collect();
    // A corridor of rooms, plus a loop back from the last to the first.
    for w in spaces.windows(2) {
        g.connect(w[0], w[1]).unwrap();
    }
    g.connect(spaces[3], spaces[0]).unwrap();
    let ledge = g.add_child(spaces[1], "ledge").unwrap();

    for h in [world, near, area] {
        g.set_envelope(h, boxed(100.0)).unwrap();
    }
    for s in &spaces {
        g.set_envelope(*s, boxed(10.0)).unwrap();
    }
    g.set_envelope(ledge, boxed(2.0)).unwrap();
    for h in [world, near, area] {
        g.advance(h, NodeState::Realized).unwrap();
    }
    for s in &spaces {
        g.advance(*s, NodeState::Realized).unwrap();
    }
    g.advance(ledge, NodeState::Realized).unwrap();

    // Reach 2 — projected only; nothing about it is committed yet.
    let far = g.add_child(world, "reach_far").unwrap();
    let far_area = g.add_child(far, "area_deep").unwrap();
    g.add_child(far_area, "space_unknown").unwrap();

    (g, near, far, spaces)
}

#[test]
fn a_partially_realized_world_is_sound() {
    let (g, near, far, _) = build_world();
    assert!(g.check_invariants().is_none(), "{:?}", g.check_invariants());
    assert_eq!(g.node(near).unwrap().state(), NodeState::Realized);
    assert_eq!(g.node(far).unwrap().state(), NodeState::Projected);
    // The realized and projected halves coexist — that *is* lazy generation.
    assert_eq!(g.find(|n| n.state() == NodeState::Realized).count(), 8);
    assert_eq!(g.find(|n| n.state() == NodeState::Projected).count(), 3);
}

#[test]
fn the_projected_reach_can_still_be_discarded() {
    let (mut g, _, far, _) = build_world();
    assert!(g.remove(far).is_ok(), "a pure forecast may be revised away");
    assert!(g.get(far).is_none());
    assert_eq!(g.of_kind(NodeKind::Reach).count(), 1);
    assert!(g.check_invariants().is_none());
}

#[test]
fn the_realized_reach_cannot_be_discarded_or_edited() {
    let (mut g, near, _, spaces) = build_world();
    assert!(matches!(
        g.remove(near),
        Err(NodeError::NotProjected { .. })
    ));
    assert!(matches!(
        g.set_name(near, "renamed"),
        Err(NodeError::Immutable { .. })
    ));
    assert!(matches!(
        g.set_envelope(spaces[0], boxed(1.0)),
        Err(NodeError::Immutable { .. })
    ));
    // Adjacency is settled during projection, so a realized Space's links are frozen too.
    assert!(matches!(
        g.connect(spaces[0], spaces[2]),
        Err(NodeError::Immutable { .. })
    ));
    assert!(matches!(
        g.disconnect(spaces[0], spaces[1]),
        Err(NodeError::Immutable { .. })
    ));
}

#[test]
fn realized_containers_still_accept_lazily_generated_children() {
    // What "Realized" freezes is a node's own attributes, not its membership — otherwise the World
    // could never be built while any region remained undiscovered.
    let (mut g, _, _, spaces) = build_world();
    let late = g
        .add_child(spaces[0], "late_ledge")
        .expect("a realized Space may gain a Spatial");
    assert_eq!(g.node(late).unwrap().state(), NodeState::Projected);
    assert_eq!(g.node(late).unwrap().kind(), NodeKind::Spatial);

    // And a whole new Reach can stream into the realized World.
    let streamed = g.add_child(g.root(), "reach_streamed").unwrap();
    let area = g.add_child(streamed, "area").unwrap();
    for h in [streamed, area] {
        g.set_envelope(h, boxed(50.0)).unwrap();
        g.advance(h, NodeState::Realized).unwrap();
    }
    assert!(g.check_invariants().is_none());
    assert_eq!(g.of_kind(NodeKind::Reach).count(), 3);
}

#[test]
fn world_round_trips_with_structure_intact() {
    let (g, near, far, spaces) = build_world();
    let bytes = to_bytes(&g);
    let back: NodeGraph = from_bytes(&bytes).expect("graph deserializes");

    assert_eq!(back, g, "round-trip must be identity");
    assert_eq!(
        to_bytes(&back),
        bytes,
        "re-serialization must be byte-stable"
    );
    assert!(back.check_invariants().is_none());

    // Handles taken before serialization still address the same nodes.
    assert_eq!(back.node(near).unwrap().name(), "reach_near");
    assert_eq!(back.node(far).unwrap().state(), NodeState::Projected);
    assert_eq!(
        back.node(spaces[0]).unwrap().neighbors(),
        g.node(spaces[0]).unwrap().neighbors()
    );
    assert_eq!(back.walk(), g.walk());
    assert_eq!(back.scale(), g.scale());
    assert_eq!(back.seed(), g.seed());
}

#[test]
fn envelopes_survive_bit_exactly() {
    let mut g = NodeGraph::new(1.0, 1);
    // A value that is not representable in binary, so any lossy path would show.
    let odd = Aabb::new(Vec3::new(0.1, -0.3, 1e-9), Vec3::new(1e9, 0.7, 123.456));
    g.set_envelope(g.root(), odd).unwrap();
    let back: NodeGraph = from_bytes(&to_bytes(&g)).unwrap();
    let got = back.node(back.root()).unwrap().envelope().unwrap();
    assert_eq!(got.min.x.to_bits(), odd.min.x.to_bits());
    assert_eq!(got.max.z.to_bits(), odd.max.z.to_bits());
}

#[test]
fn traversal_order_is_stable_across_a_round_trip() {
    let (g, _, _, _) = build_world();
    let back: NodeGraph = from_bytes(&to_bytes(&g)).unwrap();
    // Order matters: generation walks the tree, and a different order is a different world.
    let names = |graph: &NodeGraph| -> Vec<String> {
        graph
            .walk()
            .iter()
            .map(|h| graph.node(*h).unwrap().name().to_string())
            .collect()
    };
    assert_eq!(names(&back), names(&g));
    assert_eq!(
        names(&g),
        vec![
            "World",
            "reach_near",
            "area_entry",
            "space_0",
            "space_1",
            "ledge",
            "space_2",
            "space_3",
            "reach_far",
            "area_deep",
            "space_unknown",
        ]
    );
}

#[test]
fn building_the_same_world_twice_serializes_identically() {
    let (a, _, _, _) = build_world();
    let (b, _, _, _) = build_world();
    assert_eq!(to_bytes(&a), to_bytes(&b));
}

#[test]
fn a_corrupt_graph_is_rejected_at_load_not_discovered_later() {
    // Hand-build a stream whose one non-root node claims a parent that does not exist. Nothing in the
    // public API can produce this, but a truncated or tampered bundle could — and loading it as a
    // "valid" graph would surface as a wrong world much later, far from the cause.
    let (g, _, _, _) = build_world();
    let mut bytes = to_bytes(&g);

    // The root handle is written directly after the arena; corrupting it is the simplest reachable
    // structural break, and must be caught rather than parsed into a graph with no valid root.
    let tail = bytes.len() - 8 /*seed*/ - 8 /*scale*/ - 8 /*ids*/ - 8 /*root handle*/;
    bytes[tail] = 0xFE;
    bytes[tail + 1] = 0xFF;
    let result = from_bytes::<NodeGraph>(&bytes);
    assert!(
        matches!(
            result,
            Err(SerError::InvalidValue(_)) | Err(SerError::UnexpectedEof { .. })
        ),
        "corrupt graph should be rejected, got {result:?}"
    );
}

#[test]
fn an_empty_stream_is_not_a_graph() {
    assert!(from_bytes::<NodeGraph>(&[]).is_err());
    assert!(from_bytes::<NodeGraph>(&Writer::with_envelope().finish()).is_err());
}

#[test]
fn scope_queries_work_from_the_deepest_node() {
    let (g, near, _, spaces) = build_world();
    let ledge = *g
        .node(spaces[1])
        .unwrap()
        .children()
        .first()
        .expect("space_1 has a ledge");
    assert_eq!(g.scope_of(ledge, NodeKind::Space), Some(spaces[1]));
    assert_eq!(g.scope_of(ledge, NodeKind::Reach), Some(near));
    assert_eq!(g.scope_of(ledge, NodeKind::World), Some(g.root()));
    assert_eq!(g.depth_of(ledge), 4);
}
