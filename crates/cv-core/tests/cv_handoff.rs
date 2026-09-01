//! **The `CV_*` handoff, end to end** — the hole the coverage audit was written after.
//!
//! `05-object-model.md` §7 and `11-host.md` §7 both describe it, the second enumerating six keys the
//! descriptor carries: **role, layer, sphere, seed path, grants, ambient flags**. No milestone ever
//! picked it up, so the `CV_` prefix guarded a namespace nothing wrote and the channel was half a
//! feature — the guard without the payload.
//!
//! # The two halves the prefix exists to separate
//!
//! | Half | Written by | Purpose |
//! |---|---|---|
//! | free-form keys | the **developer** | project data reaching the host through generation |
//! | **`CV_*` keys** | the **core** | the generator's own facts, filterable *because* of the prefix |
//!
//! ⚠ Both live in **one map per object**, which is the point: a host walking the output does not have
//! to know which of five differently-shaped record types holds the fact it wants.

use cv_core::descriptor::{
    DescriptorBuilder, InstanceRecord, Placement, PlacementReason, Rationale, ScopeRef,
};
use cv_core::handoff::{keys, CoreFacts, CoreMeta};
use cv_core::meta::{MetaValue, RESERVED_PREFIX};
use cv_core::node::NodeGraph;
use cv_core::placement::Role;
use cv_core::{Fingerprint, ObjectId};

fn world() -> NodeGraph {
    let mut g = NodeGraph::new(1.0, 99);
    let root = g.root();
    let reach = g.add_child(root, "reach_0").unwrap();
    let area = g.add_child(reach, "area_0").unwrap();
    g.add_child(area, "space_0").unwrap();
    g
}

fn instance(name: &str) -> InstanceRecord {
    InstanceRecord {
        id: ObjectId::derived("instance", name),
        content: ObjectId::derived("actor", name),
        scope: ScopeRef(3),
        placement: Placement::IDENTITY,
        rationale: Rationale::new(PlacementReason::Scheduled),
        meta: Default::default(),
    }
}

// ---------------------------------------------------------------------------------------------
// The payload the design promises
// ---------------------------------------------------------------------------------------------

#[test]
fn a_generated_world_carries_the_generators_own_facts_to_the_host() {
    // ⚠ **The hole, closed.** Every one of the six keys `11-host.md` §7 enumerates reaches the
    // descriptor a host receives.
    let g = world();
    let mut b = DescriptorBuilder::new(&g, Fingerprint::from_raw(7), 99);

    b.place_with_facts(
        instance("heavy_door"),
        &CoreFacts::new()
            .role(Role::Gate)
            .layer(1)
            .sphere(2)
            .seed_path("world/reach_0/area_0#place[3]")
            .grants(ObjectId::derived("unlock", "vault"))
            .ambient("adopted"),
    );
    let world = b.finish_with_ambient(&["cycles:on".to_string()]);

    let placed = &world.instances[0];
    for key in keys::ALL {
        assert!(placed.meta.has(key), "{key} never reached the host");
    }
    assert_eq!(placed.meta.core_role(), Some("GATE"));
    assert_eq!(placed.meta.core_layer(), Some(1));
    assert_eq!(placed.meta.core_sphere(), Some(2));
    assert!(placed
        .meta
        .core_seed_path()
        .is_some_and(|p| p.contains("reach_0")));

    // And the run itself carries its own facts, so the root needs no special case.
    assert!(world.meta.has(keys::SEED_PATH));
    assert!(world.meta.has(keys::AMBIENT));
}

#[test]
fn a_host_separates_its_designers_data_from_the_generators_in_one_pass() {
    // ⚠ **The second half of the channel's purpose**, in the shape a host actually uses it: iterate
    // everything, split by prefix, no hard-coded key list that goes stale.
    let g = world();
    let mut b = DescriptorBuilder::new(&g, Fingerprint::from_raw(7), 99);

    let mut door = instance("heavy_door");
    door.meta
        .set("vo_bank", MetaValue::Int(4))
        .expect("a developer key");
    door.meta
        .set("faction", MetaValue::Text("cult".into()))
        .expect("a developer key");
    b.place_with_facts(door, &CoreFacts::new().role(Role::Gate).layer(1));

    let world = b.finish();
    let placed = &world.instances[0];

    assert_eq!(placed.meta.authored_keys(), vec!["vo_bank", "faction"]);
    assert_eq!(placed.meta.core_keys(), vec![keys::ROLE, keys::LAYER]);
    assert_eq!(placed.meta.len(), 4, "both halves, one map");

    // The filter is the prefix, not a list — so a core key added tomorrow still sorts correctly.
    assert!(placed
        .meta
        .core_keys()
        .iter()
        .all(|k| k.starts_with(RESERVED_PREFIX)));
    assert!(!placed
        .meta
        .authored_keys()
        .iter()
        .any(|k| k.starts_with(RESERVED_PREFIX)));
}

#[test]
fn a_developer_cannot_forge_a_generator_fact() {
    // ⚠ The guard and the payload are the same feature seen from two sides: the prefix is only a
    // trustworthy filter if content cannot write into it.
    let mut door = instance("heavy_door");
    assert!(door
        .meta
        .set(keys::ROLE, MetaValue::Text("DECORATION".into()))
        .is_err());
    assert!(door.meta.set("CV_anything", MetaValue::Bool(true)).is_err());
    assert!(door.meta.core_keys().is_empty());
}

#[test]
fn an_undetermined_fact_writes_no_key_rather_than_a_zero() {
    // ⚠ A run that never ran the sphere ladder has no sphere. Stamping `0` would tell a host *"sphere
    // zero"* — a specific, wrong claim that reads exactly like a real answer.
    let g = world();
    let mut b = DescriptorBuilder::new(&g, Fingerprint::from_raw(7), 99);
    b.place_with_facts(instance("rock"), &CoreFacts::new().role(Role::Decoration));
    let world = b.finish();

    let placed = &world.instances[0];
    assert_eq!(placed.meta.core_role(), Some("DECORATION"));
    assert_eq!(placed.meta.core_sphere(), None);
    assert!(!placed.meta.has(keys::SPHERE));
}

#[test]
fn the_facts_survive_the_wire_to_the_host() {
    // The descriptor is what crosses the seam; metadata that did not serialize would be a channel
    // that works only in-process.
    use cv_core::serialize::{from_bytes, to_bytes};

    let g = world();
    let mut b = DescriptorBuilder::new(&g, Fingerprint::from_raw(7), 99);
    let mut door = instance("heavy_door");
    door.meta
        .set("faction", MetaValue::Text("cult".into()))
        .unwrap();
    b.place_with_facts(door, &CoreFacts::new().role(Role::Gate).sphere(2));
    let world = b.finish_with_ambient(&["cycles:on".to_string()]);

    let back: cv_core::descriptor::WorldDescriptor = from_bytes(&to_bytes(&world)).unwrap();
    let placed = &back.instances[0];
    assert_eq!(placed.meta.core_role(), Some("GATE"));
    assert_eq!(placed.meta.core_sphere(), Some(2));
    assert_eq!(placed.meta.authored_keys(), vec!["faction"]);
    assert!(back.meta.has(keys::AMBIENT), "run-level facts too");
}

#[test]
fn placing_without_facts_still_works_and_stamps_nothing() {
    // ⚠ Not every write site knows every fact, and a builder that *required* them would push callers
    // into inventing values — the exact failure the optional fields exist to prevent.
    let g = world();
    let mut b = DescriptorBuilder::new(&g, Fingerprint::from_raw(7), 99);
    b.place(instance("rock"));
    let world = b.finish();
    assert!(world.instances[0].meta.core_keys().is_empty());
}
