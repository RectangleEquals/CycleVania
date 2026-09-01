//! M06 exit criteria, from the consumer's side.
//!
//! The descriptor's whole job is to be *walkable by someone who knows nothing about the engine*. So
//! rather than asserting on its fields, this file implements a **stub host**: it walks the descriptor
//! exactly as a game engine would — top-down, one pass, resolving content references against the
//! registry — and builds its own representation. If the schema were awkward to consume, that would
//! show up here as awkward code rather than as a passing test.
//!
//! What it pins:
//!
//! * a single top-down pass works (every parent precedes its children)
//! * every content reference resolves, and unresolved ones are reported *before* the walk
//! * mesh records carry metadata and rationale, and **no geometry**
//! * mirrored placements are flagged so winding can be reversed
//! * the event stream is identical between runs of the same recipe and seed

use cv_core::serialize::{from_bytes, to_bytes};
use cv_core::{
    ContentKind, ContentRegistry, DescriptorBuilder, EventLog, Fingerprint, FingerprintBuilder,
    GenEvent, InstanceRecord, MeshRecord, NodeGraph, NodeKind, NodeState, ObjectId, Placement,
    PlacementReason, Rationale, ScopeRef, Socket, Verbosity, WorldDescriptor,
};
use cv_determinism::{Aabb, Quat, Transform, Vec3};

// ---------------------------------------------------------------------------------------------
// A fixture world
// ---------------------------------------------------------------------------------------------

fn registry() -> ContentRegistry {
    let mut r = ContentRegistry::new();
    r.register(ContentKind::Actor, "crawler/door_heavy", 0x1001)
        .unwrap();
    r.register(ContentKind::Item, "crawler/key_bronze", 0x1002)
        .unwrap();
    r.register(ContentKind::StaticMesh, "kit/door_a", 0x1003)
        .unwrap();
    r.register(ContentKind::StaticMesh, "kit/corridor", 0x1004)
        .unwrap();
    r.register(ContentKind::SurfaceProperty, "portalable", 0x1005)
        .unwrap();
    r
}

/// Generate a small world, emitting events as a real run would.
fn generate(reg: &ContentRegistry, seed: u64, log: &mut EventLog) -> WorldDescriptor {
    let fingerprint: Fingerprint = FingerprintBuilder::new("0.1.0")
        .content(reg)
        .config_f64("worldScale", 1.0)
        .finish();
    log.emit(GenEvent::Started { fingerprint, seed });

    let mut g = NodeGraph::new(1.0, seed);
    let world = g.root();
    let reach = g.add_child(world, "reach_entry").unwrap();
    let area = g.add_child(reach, "area_caves").unwrap();
    let spaces: Vec<_> = (0..3)
        .map(|i| g.add_child(area, format!("space_{i}")).unwrap())
        .collect();
    let ledge = g.add_child(spaces[1], "ledge").unwrap();
    for w in spaces.windows(2) {
        g.connect(w[0], w[1]).unwrap();
    }

    let mut built = vec![world, reach, area];
    built.extend(&spaces);
    built.push(ledge);
    for h in &built {
        g.set_envelope(*h, Aabb::new(Vec3::ZERO, Vec3::splat(20.0)))
            .unwrap();
    }
    // Realize everything except the last space, which stays a forecast.
    for h in built.iter().filter(|h| **h != spaces[2]) {
        g.advance(*h, NodeState::Realized).unwrap();
    }

    let mut b = DescriptorBuilder::new(&g, fingerprint, seed);
    let door = ContentRegistry::id_for(ContentKind::Actor, "crawler/door_heavy");
    let key = ContentRegistry::id_for(ContentKind::Item, "crawler/key_bronze");
    let door_mesh = ContentRegistry::id_for(ContentKind::StaticMesh, "kit/door_a");
    let corridor_mesh = ContentRegistry::id_for(ContentKind::StaticMesh, "kit/corridor");
    let portalable = ContentRegistry::id_for(ContentKind::SurfaceProperty, "portalable");

    let s0 = b.scope_ref(spaces[0]).unwrap();
    let s1 = b.scope_ref(spaces[1]).unwrap();

    // A gate the solver required, and the key that opens it — deliberately in different rooms.
    b.place(InstanceRecord {
        id: ObjectId::derived("instance", "door_1"),
        content: door,
        scope: s0,
        placement: Placement::Trs(Transform::from_translation(Vec3::new(2.0, 0.0, 0.0))),
        rationale: Rationale::detailed(
            PlacementReason::SolverRequired,
            "gate on edge space_0→space_1",
        ),
        meta: Default::default(),
    });
    log.emit(GenEvent::Placed {
        instance: ObjectId::derived("instance", "door_1"),
        content: door,
        scope: s0,
    });

    b.place(InstanceRecord {
        id: ObjectId::derived("instance", "key_1"),
        content: key,
        scope: s1,
        placement: Placement::Trs(Transform::from_translation(Vec3::new(-1.0, 1.0, 0.5))),
        rationale: Rationale::detailed(PlacementReason::SolverRequired, "opens door_1"),
        meta: Default::default(),
    });
    log.emit(GenEvent::Placed {
        instance: ObjectId::derived("instance", "key_1"),
        content: key,
        scope: s1,
    });

    // The door's mesh, plus a *mirrored* corridor — the handed variant of the same kit piece.
    b.place_mesh(MeshRecord {
        id: ObjectId::derived("meshinst", "door_1_mesh"),
        mesh: door_mesh,
        scope: s0,
        placement: Placement::Trs(Transform::new(
            Vec3::new(2.0, 0.0, 0.0),
            Quat::from_axis_angle(Vec3::Z, 1.57),
            Vec3::ONE,
        )),
        collision: vec![Aabb::new(Vec3::ZERO, Vec3::new(1.0, 0.2, 2.5))],
        sockets: vec![Socket {
            name: "hinge".into(),
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 1.25)),
        }],
        tags: vec![portalable],
        rationale: Rationale::new(PlacementReason::Connector),
    });
    b.place_mesh(MeshRecord {
        id: ObjectId::derived("meshinst", "corridor_mirrored"),
        mesh: corridor_mesh,
        scope: s1,
        // Negative X scale: the same kit piece, handed the other way.
        placement: Placement::Trs(Transform::from_scale(Vec3::new(-1.0, 1.0, 1.0))),
        collision: vec![Aabb::new(Vec3::ZERO, Vec3::new(4.0, 3.0, 3.0))],
        sockets: Vec::new(),
        tags: Vec::new(),
        rationale: Rationale::detailed(PlacementReason::Dressing, "handed variant for kit reuse"),
    });

    let d = b.finish();
    log.emit(GenEvent::Finished {
        instances: d.instances.len() as u32,
        meshes: d.meshes.len() as u32,
    });
    d
}

// ---------------------------------------------------------------------------------------------
// A stub host
// ---------------------------------------------------------------------------------------------

/// What a game engine ends up with after consuming a descriptor.
#[derive(Debug, Default)]
struct StubHost {
    /// Room names in the order they were built.
    built: Vec<String>,
    /// World-space positions the host computed by composing each scope with its parent.
    origins: Vec<(String, Vec3)>,
    /// Assets it would need to load, deduplicated.
    assets_needed: Vec<String>,
    /// Meshes whose triangle winding must be reversed.
    winding_flips: Vec<String>,
    /// Rooms it deliberately skipped because they are still forecasts.
    deferred: Vec<String>,
}

impl StubHost {
    /// Walk a descriptor exactly once, top-down — the pass a real engine would do at load.
    fn consume(descriptor: &WorldDescriptor, registry: &ContentRegistry) -> StubHost {
        let mut host = StubHost::default();

        // A host is entitled to assume references resolve; check once, up front.
        assert!(
            descriptor.unresolved_content(registry).is_empty(),
            "every content reference should resolve before the walk begins"
        );

        // Single pass. Because parents precede children, the parent's world origin is always already
        // known by the time a child is reached — no second pass, no deferred fix-up.
        let mut world_origin: Vec<Vec3> = Vec::with_capacity(descriptor.scopes.len());
        for (i, scope) in descriptor.scopes.iter().enumerate() {
            let local = scope.envelope.map(|e| e.min).unwrap_or(Vec3::ZERO);
            let origin = match scope.parent {
                Some(p) => {
                    assert!(p.index() < i, "parent must already be processed");
                    world_origin[p.index()] + local
                }
                None => local,
            };
            world_origin.push(origin);

            if scope.state == NodeState::Realized {
                host.built.push(scope.name.clone());
                host.origins.push((scope.name.clone(), origin));
            } else {
                host.deferred.push(scope.name.clone());
            }
        }

        // Resolve what to load. The host only ever sees *references* — it loads its own assets.
        for inst in &descriptor.instances {
            let entry = registry.entry(inst.content).expect("checked above");
            let path = entry.path().to_string();
            if !host.assets_needed.contains(&path) {
                host.assets_needed.push(path);
            }
        }
        for mesh in &descriptor.meshes {
            let entry = registry.entry(mesh.mesh).expect("checked above");
            let path = entry.path().to_string();
            if !host.assets_needed.contains(&path) {
                host.assets_needed.push(path.clone());
            }
            if mesh.needs_winding_flip() {
                host.winding_flips.push(path);
            }
        }
        host
    }
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[test]
fn a_stub_host_can_walk_a_world_in_one_top_down_pass() {
    let reg = registry();
    let mut log = EventLog::new();
    let d = generate(&reg, 42, &mut log);
    assert!(d.check().is_none(), "{:?}", d.check());

    let host = StubHost::consume(&d, &reg);
    assert_eq!(
        host.built,
        vec![
            "World",
            "reach_entry",
            "area_caves",
            "space_0",
            "space_1",
            "ledge"
        ]
    );
    // The unrealized room was skipped, not guessed at.
    assert_eq!(host.deferred, vec!["space_2"]);
    // Nested scopes accumulated their parents' origins during that single pass.
    let ledge = host.origins.iter().find(|(n, _)| n == "ledge").unwrap();
    assert_eq!(
        ledge.1,
        Vec3::ZERO,
        "envelopes all start at the origin in this fixture"
    );
}

#[test]
fn the_host_loads_assets_by_reference_and_is_told_about_winding() {
    let reg = registry();
    let mut log = EventLog::new();
    let d = generate(&reg, 42, &mut log);
    let host = StubHost::consume(&d, &reg);

    assert_eq!(
        host.assets_needed,
        vec![
            "crawler/door_heavy",
            "crawler/key_bronze",
            "kit/door_a",
            "kit/corridor"
        ]
    );
    // The mirrored corridor — and only it — needs its winding reversed.
    assert_eq!(host.winding_flips, vec!["kit/corridor"]);
}

#[test]
fn a_descriptor_carries_no_geometry_however_big_the_world() {
    // The size claim that makes the boundary worth having: output scales with *decisions*, not with
    // art. A world referencing a cathedral costs the same as one referencing a crate.
    let reg = registry();
    let mut log = EventLog::new();
    let d = generate(&reg, 42, &mut log);
    let bytes = to_bytes(&d);
    assert!(
        bytes.len() < 4096,
        "a 7-scope world should be well under 4 KB, got {} bytes",
        bytes.len()
    );

    // Every mesh reference is an id, and every "shape" is a coarse volume — never vertices.
    for m in &d.meshes {
        assert!(
            reg.contains(m.mesh),
            "meshes are referenced, never embedded"
        );
        assert!(
            m.collision.len() <= 4,
            "collision is coarse metadata, not a collision mesh"
        );
    }
}

#[test]
fn every_placement_explains_itself() {
    let reg = registry();
    let mut log = EventLog::new();
    let d = generate(&reg, 42, &mut log);

    // The question a dev asks constantly — "why is this here?" — is answerable from the output alone.
    let door = d
        .instances
        .iter()
        .find(|i| i.id == ObjectId::derived("instance", "door_1"))
        .unwrap();
    assert_eq!(door.rationale.reason, PlacementReason::SolverRequired);
    assert!(door.rationale.detail.contains("gate on edge"));
    assert!(
        door.rationale.reason.is_required(),
        "removing this would break solvability"
    );

    let dressing = d
        .meshes
        .iter()
        .find(|m| m.rationale.reason == PlacementReason::Dressing)
        .unwrap();
    assert!(
        !dressing.rationale.reason.is_required(),
        "dressing is safe to cull"
    );
}

#[test]
fn the_descriptor_survives_the_trip_to_a_host() {
    let reg = registry();
    let mut log = EventLog::new();
    let d = generate(&reg, 42, &mut log);

    // Serialize, ship, reload — and the host gets the identical world.
    let back: WorldDescriptor = from_bytes(&to_bytes(&d)).expect("descriptor loads");
    assert_eq!(back, d);
    assert_eq!(
        format!("{:?}", StubHost::consume(&back, &reg)),
        format!("{:?}", StubHost::consume(&d, &reg))
    );
}

#[test]
fn generation_is_reproducible_and_so_is_its_event_stream() {
    let reg = registry();

    let mut log_a = EventLog::with_verbosity(Verbosity::Verbose);
    let a = generate(&reg, 7, &mut log_a);
    let mut log_b = EventLog::with_verbosity(Verbosity::Verbose);
    let b = generate(&reg, 7, &mut log_b);

    assert_eq!(
        to_bytes(&a),
        to_bytes(&b),
        "same recipe and seed ⇒ same world"
    );
    assert_eq!(
        to_bytes(&log_a.drain()),
        to_bytes(&log_b.drain()),
        "the trace must be diffable between runs, so its order cannot vary"
    );
}

#[test]
fn a_host_can_ask_which_rooms_are_ready_to_build() {
    let reg = registry();
    let mut log = EventLog::new();
    let d = generate(&reg, 42, &mut log);

    // Lazy generation, from the consumer's side: build what exists, come back for the rest.
    let ready: Vec<&str> = d.realized_scopes().map(|(_, s)| s.name.as_str()).collect();
    assert!(ready.contains(&"space_0"));
    assert!(!ready.contains(&"space_2"));
    assert_eq!(d.scopes_of_kind(NodeKind::Space).count(), 3);
    assert_eq!(d.realized_scopes().count(), 6);

    // And it can find what belongs in a given room without scanning the whole world by hand.
    let s0 = ScopeRef(3);
    assert_eq!(d.instances_in(s0).count(), 1);
    assert_eq!(d.meshes_in(s0).count(), 1);
}
