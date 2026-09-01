//! M09a exit criteria: **a distributed puzzle places as a cohort, and a `debug_only` message is
//! absent from a cooked build.**
//!
//! Three small surfaces with almost no code between them, each load-bearing for something later.
//!
//! # Why "absent" and not "suppressed"
//!
//! ⚠ A suppressed message still carries its text through the build; a **stripped** one is never
//! constructed. That difference is exactly why `debug_only` defaults to `true` — a developer who never
//! thinks about it ships nothing rather than everything, and the failure mode of forgetting is silence
//! rather than a debug string in a player's log.

use cv_core::events::{EventLog, GenEvent, Verbosity};
use cv_core::meta::{check_key, MetaError, MetaValue, Metadata, RESERVED_PREFIX};
use cv_core::node::InstanceScope;
use cv_core::placement::Constraint;
use cv_core::{ClassPath, Object, ObjectId};
use cv_determinism::Vec3;

fn class(p: &str) -> ClassPath {
    ClassPath::new(p).unwrap()
}

// ---------------------------------------------------------------------------------------------
// P01 — a distributed puzzle places as a cohort
// ---------------------------------------------------------------------------------------------

#[test]
fn a_distributed_puzzle_is_one_constraint_over_separate_placeables() {
    // ⚠ Four levers that must be in one room with the door they open. They are **genuinely separate
    // placeables** — each gets its own position — which is the case co-locating as components of one
    // Actor cannot cover.
    let members = [
        class("/Content/Puzzles/LeverA"),
        class("/Content/Puzzles/LeverB"),
        class("/Content/Puzzles/LeverC"),
        class("/Content/Puzzles/SealedDoor"),
    ];
    let c = Constraint::cohort(members.iter().cloned(), InstanceScope::Space);

    let Constraint::Cohort {
        members: m,
        scope,
        all_or_nothing,
        ordered,
    } = &c
    else {
        panic!("a cohort");
    };
    assert_eq!(m.len(), 4);
    assert_eq!(*scope, InstanceScope::Space);
    assert!(*all_or_nothing, "the default, and the important one");
    assert!(
        !*ordered,
        "ordering constrains hard and most cohorts skip it"
    );
}

#[test]
fn all_or_nothing_defaults_true_because_half_a_puzzle_is_worse_than_none() {
    // ⚠ The player finds three of the four levers and no fourth exists. Under scarcity the right
    // outcome is *no puzzle*, not *a broken puzzle* — and a default of `false` would produce the
    // second every time content ran short.
    let c = Constraint::cohort(
        [class("/Content/A"), class("/Content/B")],
        InstanceScope::Area,
    );
    let Constraint::Cohort { all_or_nothing, .. } = c else {
        panic!("a cohort")
    };
    assert!(all_or_nothing);
}

#[test]
fn a_cohort_can_span_the_world_when_the_pieces_belong_apart() {
    // Twelve markers paired with twelve World-scattered items: the scope is what says how wide
    // *together* is, and it goes all the way out.
    let members: Vec<ClassPath> = (0..12)
        .map(|i| class(&format!("/Content/Markers/M{i}")))
        .collect();
    let c = Constraint::cohort(members, InstanceScope::World);
    let Constraint::Cohort { members, scope, .. } = &c else {
        panic!("a cohort")
    };
    assert_eq!(members.len(), 12);
    assert_eq!(*scope, InstanceScope::World);
}

#[test]
fn a_cohort_explains_itself_for_a_trace() {
    let c = Constraint::cohort(
        [
            class("/Content/A"),
            class("/Content/B"),
            class("/Content/C"),
        ],
        InstanceScope::Space,
    );
    let text = c.to_string();
    assert!(text.contains('2'), "the other two members: {text}");
    assert!(text.contains("Space"), "{text}");
}

// ---------------------------------------------------------------------------------------------
// P03 — a `debug_only` message is absent from a cooked build
// ---------------------------------------------------------------------------------------------

fn debug_msg() -> GenEvent {
    GenEvent::Message {
        text: "lever budget = 4".into(),
        channel: "puzzle".into(),
        debug_only: true,
    }
}

fn shipped_msg() -> GenEvent {
    GenEvent::Message {
        text: "sealed door placed".into(),
        channel: "puzzle".into(),
        debug_only: false,
    }
}

#[test]
fn a_debug_message_is_absent_from_a_cooked_build() {
    // ⚠ **The milestone's green criterion.** Not "filtered out on the way to a listener" — absent.
    let mut cooked = EventLog::cooked();
    cooked.emit(debug_msg());
    cooked.emit(shipped_msg());

    assert!(cooked.is_cooked());
    let texts: Vec<String> = cooked.drain().iter().map(GenEvent::to_string).collect();
    assert_eq!(texts.len(), 1);
    assert!(
        !texts.iter().any(|t| t.contains("lever budget")),
        "a debug string reached a cooked build: {texts:?}"
    );
    assert!(texts[0].contains("sealed door placed"));
}

#[test]
fn a_stripped_message_is_not_reported_as_suppressed() {
    // ⚠ A cooked build has no debug messages to have dropped. Counting one would be telling a
    // player's log that something was withheld from it, which is a different and untrue statement.
    let mut cooked = EventLog::cooked();
    cooked.emit(debug_msg());
    assert_eq!(cooked.suppressed(), 0);
}

#[test]
fn the_same_message_survives_an_uncooked_build() {
    // The control: stripping must be a property of the *build*, not of the message.
    let mut dev = EventLog::with_verbosity(Verbosity::Coarse);
    dev.emit(debug_msg());
    assert_eq!(dev.drain().len(), 1);
}

#[test]
fn a_message_routes_under_its_channel_so_a_host_subscribes_directly() {
    // ⚠ Not under `"message"` with the host re-dispatching — the channel *is* the subscription name.
    assert_eq!(shipped_msg().name(), "puzzle");
}

#[test]
fn the_host_boundary_is_one_way_by_construction() {
    // ⚠ `emit` takes an event and returns nothing. A host reply that changed the solve would kill
    // replayability, and there is no signature here that could carry one.
    //
    // Stated as a type: a function returning `()` cannot carry an answer, so the one-wayness is not a
    // rule anyone is asked to respect.
    fn assert_one_way<F: Fn(&mut EventLog, GenEvent)>(_emit: F) {}
    assert_one_way(EventLog::emit);

    let mut log = EventLog::new();
    log.emit(shipped_msg());
    assert_eq!(log.drain().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// P02 — metadata, and the `CV_` guard
// ---------------------------------------------------------------------------------------------

/// A minimal object, so the trait's metadata methods are exercised through the real path.
#[derive(Clone, Debug)]
struct Marker(cv_core::ObjectHeader);

impl Object for Marker {
    fn header(&self) -> &cv_core::ObjectHeader {
        &self.0
    }
    fn header_mut(&mut self) -> &mut cv_core::ObjectHeader {
        &mut self.0
    }
    fn type_name(&self) -> &'static str {
        "Marker"
    }
}

#[test]
fn every_object_carries_metadata_and_the_core_namespace_is_refused() {
    // ⚠ The escape hatch that is not an escape from determinism: a project always has one fact the
    // core did not model, and the alternative to a metadata channel is a project fork.
    let mut m = Marker(cv_core::ObjectHeader::derived("actor", "lever_a"));

    assert!(m
        .set_meta("faction", MetaValue::Text("cult".into()))
        .is_ok());
    assert_eq!(m.meta("faction").and_then(MetaValue::as_text), Some("cult"));
    assert!(m.has_meta("faction"));
    assert_eq!(m.meta_keys(), vec!["faction"]);

    // The runtime layer of the guard: a computed key no compiler ever saw.
    let computed = format!("{RESERVED_PREFIX}{}", "rationale");
    assert!(matches!(
        m.set_meta(&computed, MetaValue::Bool(true)),
        Err(MetaError::Reserved { .. })
    ));

    assert!(m.remove_meta("faction"));
    assert!(!m.has_meta("faction"));
}

#[test]
fn the_guard_is_one_function_the_three_layers_share() {
    // ⚠ Three checks written separately would eventually disagree, and the one that was wrong would
    // be whichever route the accident took. The binding boundary calls exactly this.
    assert!(check_key("CV_anything").is_err());
    assert!(check_key("").is_err());
    assert!(check_key("my_CV_note").is_ok(), "prefix, not substring");
}

#[test]
fn metadata_is_insertion_ordered_in_memory_and_sorted_on_the_wire() {
    use cv_core::serialize::to_bytes;

    let mut a = Metadata::new();
    a.set("zebra", MetaValue::Int(1)).unwrap();
    a.set("apple", MetaValue::Vec3(Vec3::new(1.0, 0.0, 0.0)))
        .unwrap();

    let mut b = Metadata::new();
    b.set("apple", MetaValue::Vec3(Vec3::new(1.0, 0.0, 0.0)))
        .unwrap();
    b.set("zebra", MetaValue::Int(1)).unwrap();

    assert_ne!(
        a.keys().collect::<Vec<_>>(),
        b.keys().collect::<Vec<_>>(),
        "the inspector shows what each developer typed"
    );
    assert_eq!(to_bytes(&a), to_bytes(&b), "but the recipe is the same");
}

#[test]
fn metadata_travels_with_the_object_across_the_wire() {
    use cv_core::serialize::{from_bytes, to_bytes};

    let mut m = Marker(cv_core::ObjectHeader::derived("actor", "lever_a"));
    m.set_meta("weight", MetaValue::Float(2.5)).unwrap();
    m.set_meta("linked", MetaValue::Ref(ObjectId::derived("actor", "door")))
        .unwrap();

    let back: cv_core::ObjectHeader = from_bytes(&to_bytes(m.header())).unwrap();
    assert_eq!(
        back.meta.get("weight").and_then(MetaValue::as_float),
        Some(2.5)
    );
    assert!(back.meta.has("linked"));
}

// ---------------------------------------------------------------------------------------------
// P04 — two channels, and there is no third
// ---------------------------------------------------------------------------------------------

#[test]
fn there_is_no_component_to_owner_signal_channel() {
    // ⚠ **Stated as a prohibition rather than an omission.** A component is owned by its schematic, so
    // its graph calls its owner's function; a signal channel would let a component fire into the void
    // and make ordering unanalysable.
    //
    // The check that can be made in code: the *only* event a component's behaviour can produce is a
    // host-facing `Message`, and nothing in `GenEvent` addresses an owner.
    let events = [
        GenEvent::Message {
            text: String::new(),
            channel: "c".into(),
            debug_only: false,
        },
        GenEvent::Finished {
            instances: 0,
            meshes: 0,
        },
    ];
    let names: Vec<&str> = events.iter().map(GenEvent::name).collect();
    assert_eq!(names, vec!["c", "finished"]);
}

#[test]
fn silent_costs_nothing_when_nobody_is_listening() {
    // Channel 1 is observational, so an unsubscribed run must not pay for the text it would have sent.
    let mut log = EventLog::with_verbosity(Verbosity::Silent);
    for _ in 0..1_000 {
        log.emit(shipped_msg());
    }
    assert_eq!(log.suppressed(), 1_000);
    assert!(log.drain().is_empty());
}
