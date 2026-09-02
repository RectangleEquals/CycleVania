//! M08 exit criteria: **a class default is read with nothing instantiated, and a `Kind` never wires
//! into a `Ref`.**
//!
//! > **Green when:** `Kind<T>.defaults()` reads an authored field off a schematic with nothing
//! > instantiated, and `Kind<T>` cannot be wired to a `Ref<T>` pin.
//!
//! # Why the first half is the milestone
//!
//! ⚠ **Constructing an instance to mean a kind is the bug that caused the visual-authoring pivot.** It
//! is a bug that *works* — the object carries the right fields and answers the right questions — right
//! up until two mentions of "the same thing" turn out to be two different objects. Reading a class's
//! authored values without building one is what closes it, and the property that makes the close real
//! is that the default's id is **derived from the path** rather than allocated.
//!
//! ⚠ **The lattice does not appear here.** Unlocks are table rows (M03a); they trade in neither
//! instances nor classes. This machinery is for the many places that genuinely name a class.

use cv_core::class::{ActorBound, ComponentBound, CoreClass, ItemBound, SurfaceBound};
use cv_core::component::{Attached, CollisionMode, Component, Components};
use cv_core::mission::Rule;
use cv_core::shape::Shape;
use cv_core::{
    AssetPath, ClassError, ClassPath, ClassRegistry, FieldValue, InstanceScope, Kind, PinType, Ref,
    ResourceRef,
};
use cv_determinism::Vec3;

fn class(p: &str) -> ClassPath {
    ClassPath::new(p).unwrap()
}

/// A small project: two authored items, an authored surface, and a resource class.
fn project() -> ClassRegistry {
    let mut r = ClassRegistry::with_core();
    // ⚠ `/Core/MeshResource` and the rest of the tier-1 tree come with `with_core` — a project never
    // re-declares the core, or two projects would eventually disagree about what extends what.
    r.register(class("/Content/Surfaces/Stone"), class("/Core/Surface"))
        .unwrap();
    r.register(class("/Content/Items/Hookshot"), class("/Core/Item"))
        .unwrap();
    r.register(
        class("/Content/Items/Longshot"),
        class("/Content/Items/Hookshot"),
    )
    .unwrap();
    r.register(
        class("/Content/Components/TetherComponent"),
        class("/Core/Component"),
    )
    .unwrap();

    // Authored on the schematic, the way a `.cvs` file's `Begin Component … Length=…` block is.
    r.author(
        &class("/Content/Items/Hookshot"),
        "range",
        FieldValue::Number(30.0),
    )
    .unwrap();
    r.author(
        &class("/Content/Items/Hookshot"),
        "mesh",
        FieldValue::Asset(AssetPath::new("/Content/Meshes/hookshot.glb").unwrap()),
    )
    .unwrap();
    r.author(
        &class("/Content/Items/Hookshot"),
        "tether",
        FieldValue::Class(class("/Content/Components/TetherComponent")),
    )
    .unwrap();
    r.author(
        &class("/Content/Items/Longshot"),
        "range",
        FieldValue::Number(60.0),
    )
    .unwrap();
    r
}

// ---------------------------------------------------------------------------------------------
// The criterion
// ---------------------------------------------------------------------------------------------

#[test]
fn a_schematics_authored_fields_are_read_without_building_one() {
    // ⚠ **The milestone's green criterion.** Nothing is constructed anywhere in this test — the
    // registry holds classes, and `defaults()` names the one core-owned object each of them owns.
    let r = project();
    let hookshot = Kind::<ItemBound>::new(&r, class("/Content/Items/Hookshot")).unwrap();

    assert_eq!(
        hookshot
            .default_field(&r, "range")
            .and_then(FieldValue::as_number),
        Some(30.0)
    );
    assert_eq!(
        hookshot
            .default_field(&r, "tether")
            .and_then(FieldValue::as_class),
        Some(&class("/Content/Components/TetherComponent"))
    );

    // The default is a `Ref`, and it is the *same* Ref every time anyone asks.
    let a: Ref<ItemBound> = hookshot.defaults();
    let b: Ref<ItemBound> = Kind::<ItemBound>::new(&r, class("/Content/Items/Hookshot"))
        .unwrap()
        .defaults();
    assert_eq!(a, b, "one core-owned object per class, not one per lookup");
}

#[test]
fn a_kind_never_wires_into_a_ref_and_a_ref_never_into_a_kind() {
    // ⚠ **The other half.** Not a subtyping failure that happens to be caught — *"which class"* has no
    // reading as *"which one of them"*, so the connection is meaningless in both directions.
    let r = project();
    let item_kind = PinType::Kind(class("/Core/Item"));
    let item_ref = PinType::Ref(class("/Core/Item"));

    assert!(!item_kind.accepts(&item_ref, &r));
    assert!(!item_ref.accepts(&item_kind, &r));

    // And a resource pin takes neither.
    let res = PinType::Resource(class("/Core/MeshResource"));
    assert!(!res.accepts(&item_kind, &r));
    assert!(!res.accepts(&item_ref, &r));
}

#[test]
fn the_default_id_is_derived_so_two_processes_agree_on_it() {
    // ⚠ An *allocated* id would differ between runs, and two readers of "the same class default" would
    // silently disagree — the exact shape of the bug this machinery exists to close.
    let r = project();
    let from_registry = Kind::<ItemBound>::new(&r, class("/Content/Items/Hookshot"))
        .unwrap()
        .defaults()
        .id();
    let from_path = cv_core::class::class_default_id(&class("/Content/Items/Hookshot"));
    assert_eq!(from_registry, from_path);

    // Two registries built independently still agree.
    let r2 = project();
    assert_eq!(
        Kind::<ItemBound>::new(&r2, class("/Content/Items/Hookshot"))
            .unwrap()
            .defaults()
            .id(),
        from_registry
    );
}

// ---------------------------------------------------------------------------------------------
// The picker
// ---------------------------------------------------------------------------------------------

#[test]
fn a_wrong_pick_was_never_on_the_menu() {
    // ⚠ `Kind<T>` semantics: the picker lists only subclasses of `T`, so choosing wrongly is not an
    // error a developer is told about afterwards.
    let r = project();
    let offered: Vec<&str> = r
        .subclasses_of(&class("/Core/Item"))
        .iter()
        .map(|p| p.as_str())
        .collect();
    assert_eq!(
        offered,
        vec![
            "/Content/Items/Hookshot",
            "/Content/Items/Longshot",
            "/Core/Item"
        ]
    );

    assert!(matches!(
        Kind::<ItemBound>::new(&r, class("/Content/Components/TetherComponent")),
        Err(ClassError::NotUnderBound { .. })
    ));
}

#[test]
fn the_bound_widens_upward_and_the_check_is_the_only_way_down() {
    let r = project();
    let longshot = Kind::<ItemBound>::new(&r, class("/Content/Items/Longshot")).unwrap();
    assert!(longshot.upcast::<ActorBound>(&r).is_some());

    let component =
        Kind::<ComponentBound>::new(&r, class("/Content/Components/TetherComponent")).unwrap();
    assert!(
        component.upcast::<ItemBound>(&r).is_none(),
        "a component is not an item, in any direction"
    );
}

#[test]
fn a_pin_takes_a_narrower_class_within_its_own_family() {
    let r = project();
    let actor_pin = PinType::Kind(class("/Core/Actor"));
    assert!(actor_pin.accepts(&PinType::Kind(class("/Content/Items/Longshot")), &r));
    assert!(!actor_pin.accepts(&PinType::Kind(class("/Core/Component")), &r));
}

// ---------------------------------------------------------------------------------------------
// Inheritance
// ---------------------------------------------------------------------------------------------

#[test]
fn an_override_replaces_one_field_and_inherits_the_rest() {
    // ⚠ Without this a subclass would restate every field it did not change, and schematics would
    // drift out of sync with their parents one forgotten field at a time.
    let r = project();
    let longshot = Kind::<ItemBound>::new(&r, class("/Content/Items/Longshot")).unwrap();
    assert_eq!(
        longshot
            .default_field(&r, "range")
            .and_then(FieldValue::as_number),
        Some(60.0),
        "the override wins"
    );
    assert_eq!(
        longshot
            .default_field(&r, "tether")
            .and_then(FieldValue::as_class),
        Some(&class("/Content/Components/TetherComponent")),
        "and the rest comes from the parent"
    );
    assert_eq!(
        longshot.default_field(&r, "nonexistent"),
        None,
        "an unauthored field is absent, not zero"
    );
}

// ---------------------------------------------------------------------------------------------
// Where the machinery actually lands
// ---------------------------------------------------------------------------------------------

#[test]
fn the_component_fields_the_design_types_kind_now_hold_class_paths() {
    // ⚠ **A lever must reach something.** `Kind<T>` that nothing holds would be the same defect as a
    // dial that changes nothing. These are the fields the design types `Kind<T>`, and every one of
    // them is now a class path checkable against a bound rather than an untyped id.
    let r = project();

    let surface = class("/Content/Surfaces/Stone");
    let shape = Component::Shape {
        shape: Shape::Cube {
            extents: Vec3::new(1.0, 1.0, 1.0),
            bevel: 0.0,
        },
        surface: Some(surface.clone()),
        collision_mode: CollisionMode::Exact,
        visible: true,
    };

    // The stored path is checkable against the bound the manifest declares for that field.
    let bound = PinType::Kind(SurfaceBound::class_path());
    assert!(bound.accepts(&PinType::Kind(surface.clone()), &r));

    // And a wrong one is refused by the same check, at load, rather than at use.
    assert!(!bound.accepts(&PinType::Kind(class("/Content/Items/Hookshot")), &r));

    let body = Components::new().with(Attached::new(shape));
    assert_eq!(body.collision().surfaces(), vec![surface]);
}

#[test]
fn a_checkpoint_restores_classes_and_a_barrier_matches_one() {
    let r = project();
    let checkpoint = Component::Checkpoint {
        restores: vec![class("/Content/Items/Hookshot")],
        restores_occupant: true,
        scope: InstanceScope::Area,
    };
    let Component::Checkpoint { restores, .. } = &checkpoint else {
        panic!("a checkpoint");
    };
    let object_bound = PinType::Kind(class("/Core/Object"));
    assert!(restores
        .iter()
        .all(|c| object_bound.accepts(&PinType::Kind(c.clone()), &r)));

    let bar = Component::BlocksTraversal {
        matching: class("/Content/Components/TetherComponent"),
        route: None,
    };
    let Component::BlocksTraversal { matching, .. } = &bar else {
        panic!("a barrier");
    };
    // ⚠ The design bounds this at `Kind<TraversalComponent>`; a plain component is *not* one, and the
    // bound is what says so.
    let traversal_bound = PinType::Kind(class("/Core/TraversalComponent"));
    assert!(!traversal_bound.accepts(&PinType::Kind(matching.clone()), &r));
    assert!(PinType::Kind(class("/Core/Component")).accepts(&PinType::Kind(matching.clone()), &r));
}

#[test]
fn a_rules_unlocks_and_its_classes_are_two_sets_that_never_overlap() {
    // ⚠ **The bug the retype found.** While both were `ObjectId`, `HasComponent` was answered against
    // the held-*unlock* set — comparing a component class to a lattice atom, always false, and nothing
    // caught it because the types agreed.
    let dash = cv_core::ObjectId::derived("unlock", "dash");
    let r = Rule::All(vec![
        Rule::has(dash),
        Rule::HasComponent(class("/Content/Components/TetherComponent")),
        Rule::Nearby {
            kind: class("/Content/Items/Hookshot"),
            within: cv_core::BudgetRef::distance(8.0),
            scope: InstanceScope::Space,
        },
    ]);

    assert_eq!(r.unlocks().len(), 1);
    assert!(r.unlocks().contains(&dash));
    assert_eq!(r.classes().len(), 2, "and the classes are their own set");
    assert!(r.classes().contains(&class("/Content/Items/Hookshot")));
}

#[test]
fn a_mesh_reference_is_a_class_and_a_file_and_neither_alone() {
    // ⚠ The path alone says nothing about how to read the bytes; the core loads it with *that class's*
    // loader rather than guessing from the extension.
    let r = project();
    let mesh = ResourceRef::new(
        class("/Core/MeshResource"),
        AssetPath::new("/Content/Meshes/hookshot.glb").unwrap(),
    );
    assert!(mesh.is_well_formed(&r));

    let component = Component::Mesh {
        asset: mesh.clone(),
        surfaces: [("stone".to_string(), class("/Content/Surfaces/Stone"))]
            .into_iter()
            .collect(),
        collision_mode: CollisionMode::Hull,
        visible: true,
    };
    let Component::Mesh {
        asset, surfaces, ..
    } = &component
    else {
        panic!("a mesh");
    };
    assert_eq!(asset.asset.extension().as_deref(), Some("glb"));
    assert!(PinType::Kind(SurfaceBound::class_path())
        .accepts(&PinType::Kind(surfaces["stone"].clone()), &r));

    // A class in a value position is nonsense, and the well-formedness check says so.
    let wrong = ResourceRef::new(
        class("/Core/Actor"),
        AssetPath::new("/Content/Meshes/hookshot.glb").unwrap(),
    );
    assert!(!wrong.is_well_formed(&r));
}

// ---------------------------------------------------------------------------------------------
// The registry's own rules
// ---------------------------------------------------------------------------------------------

#[test]
fn a_project_may_extend_core_and_core_may_never_extend_a_project() {
    // ⚠ Otherwise a project could move the tier-1 surface under itself, and every guarantee stated
    // about `/Core/…` would become a guarantee about whatever the project last edited.
    let mut r = project();
    assert!(r
        .register(class("/Content/Items/Grapple"), class("/Core/Item"))
        .is_ok());
    assert!(matches!(
        r.register(class("/Core/Sneaky"), class("/Content/Items/Hookshot")),
        Err(ClassError::MountViolation { .. })
    ));
}

#[test]
fn a_path_that_is_not_mounted_is_refused_before_it_can_mean_two_things() {
    // ⚠ A bare name breaks the moment a project and a preset both define one — and the failure is
    // silent, because both resolve to *something*.
    assert!(ClassPath::new("Door").is_err());
    assert!(ClassPath::new("/Items/Door").is_err());
    assert!(ClassPath::new("/Content/Items/Door").is_ok());
}
