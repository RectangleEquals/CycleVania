//! **M15's green condition** — the declarations regenerate with no diff, and a dial set from host code
//! changes the generated world, including from a cooked build.
//!
//! ⚠ **The interesting half is the cooked build.** *"Dials are inputs, not content"* is a claim, and the
//! only way to check it is to cook a project and set one anyway. A generator where cooking quietly froze
//! them would pass every other test in this repository.

use cv_bindings::{
    DialBounds, DialKind, DialMeta, DialSource, DialValue, GenerateOptions, Project,
};

/// The generated declarations, as they sit on disk.
fn declarations() -> String {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("index.d.ts");
    std::fs::read_to_string(path).expect("the generated declarations are committed")
}

#[test]
fn every_dial_kind_is_spelled_the_same_in_rust_and_in_typescript() {
    // ⚠ Two spellings of one enum drift, and the first anyone hears of it is a host reading "CURVE"
    // and getting nothing.
    let ts = declarations();
    for kind in DialKind::ALL {
        assert!(
            ts.contains(&format!("\"{}\"", kind.name())),
            "{} is missing from the declarations",
            kind.name()
        );
    }
    assert!(
        ts.contains(r#"export type DialKind = "NUMBER" | "RANGE" | "ADAPTIVE" | "ENUM" | "CURVE" | "TABLE";"#),
        "the union must list exactly the Rust variants, in order"
    );
}

#[test]
fn every_dial_source_is_spelled_the_same_on_both_sides() {
    let ts = declarations();
    for source in [DialSource::Authored, DialSource::Host, DialSource::Scoped] {
        assert!(
            ts.contains(&format!("\"{}\"", source.name())),
            "{} is missing",
            source.name()
        );
    }
}

#[test]
fn the_declarations_carry_both_default_and_effective() {
    // ⚠ Neither is derivable from the other: only-effective makes *reset* impossible, only-default
    // makes the API a lie about what the next generate uses.
    let ts = declarations();
    assert!(ts.contains("  default: DialValue;"));
    assert!(ts.contains("  effective: DialValue;"));
    assert!(ts.contains("  source: DialSource;"));
}

#[test]
fn the_declarations_expose_set_and_set_source_as_two_calls() {
    // ⚠ Swapping a constant for a curve changes the dial's kind, which `set` cannot express.
    let ts = declarations();
    assert!(ts.contains("set(id: string, value: DialValue"));
    assert!(ts.contains("setSource(id: string, source: DialValue): void;"));
}

#[test]
fn the_declarations_expose_the_cooked_entry_point() {
    // ⚠ `loadFromFile` is the whole host-facing surface for a shipped game — one file, no content root.
    let ts = declarations();
    assert!(ts.contains("export function loadFromFile(path: string): Project;"));
    assert!(ts.contains("readonly cooked: boolean;"));
}

#[test]
fn the_api_surface_and_the_host_surface_are_in_one_file() {
    // ⚠ A hand-maintained companion file drifts; this is the property that stops it.
    let ts = declarations();
    assert!(ts.contains("export type DialKind"), "the host surface");
    assert!(
        ts.contains("Trivalent") || ts.contains("ObjectId"),
        "and the manifest-derived API surface"
    );
}

fn hookshot() -> DialMeta {
    DialMeta::authored(
        "/Content/Items/Hookshot",
        "length",
        DialValue::Number(30.0),
        DialBounds::number(8.0, 200.0),
    )
    .documented("how far the rope reaches")
}

#[test]
fn a_dial_set_from_host_code_changes_the_generated_world() {
    let mut project = Project::new("./game.cvproj");
    project.dials_mut().declare(hookshot());
    project.validate().unwrap();

    let before = project
        .generate(GenerateOptions::seeded("world-42"))
        .unwrap();

    project
        .dials_mut()
        .set("Hookshot.length", DialValue::Number(120.0), None)
        .unwrap();
    project.validate().unwrap();
    let after = project
        .generate(GenerateOptions::seeded("world-42"))
        .unwrap();

    assert_ne!(before.fingerprint, after.fingerprint);
    assert_ne!(before, after);
}

#[test]
fn a_dial_set_in_a_cooked_build_changes_the_world_too() {
    // ⚠ The case that proves dials are inputs rather than content. Nothing about cooking freezes them,
    // which is what makes this the override channel a curve table points at.
    let mut cooked = Project::load_from_file("./build/game.cvpak");
    assert!(cooked.cooked);
    cooked.dials_mut().declare(hookshot());
    cooked.validate().unwrap();

    let before = cooked.generate(GenerateOptions::seeded("shipped")).unwrap();

    assert_eq!(cooked.dials().len(), 1, "a cooked build lists its dials");
    assert_eq!(
        cooked.dials().get("Hookshot.length").unwrap().doc,
        "how far the rope reaches",
        "and their docs, because nothing here is stripped at cook"
    );

    cooked
        .dials_mut()
        .set("Hookshot.length", DialValue::Number(120.0), None)
        .unwrap();
    cooked.validate().unwrap();
    let after = cooked.generate(GenerateOptions::seeded("shipped")).unwrap();

    assert_ne!(
        before.fingerprint, after.fingerprint,
        "cooking must not freeze a dial"
    );
}

#[test]
fn a_cooked_build_can_swap_a_constant_for_a_curve_after_shipping() {
    // ⚠ The scenario the design names: tuning that must move after shipping goes through `setSource`
    // rather than through a patchable asset.
    let mut cooked = Project::load_from_file("./build/game.cvpak");
    cooked.dials_mut().declare(hookshot());
    cooked.validate().unwrap();
    let before = cooked.fingerprint();

    cooked
        .dials_mut()
        .set_source(
            "Hookshot.length",
            DialValue::Curve {
                asset: "/Content/Curves/wear.cvcurve".into(),
                row: "rate".into(),
            },
        )
        .unwrap();

    let meta = cooked.dials().get("Hookshot.length").unwrap();
    assert_eq!(meta.kind, DialKind::Curve);
    assert_eq!(meta.source, DialSource::Host);
    assert_ne!(cooked.fingerprint(), before);
}

#[test]
fn all_six_value_kinds_survive_a_round_trip_through_the_surface() {
    let mut project = Project::new("./game.cvproj");
    let cases: [(&str, DialValue, DialBounds); 6] = [
        ("n", DialValue::Number(1.0), DialBounds::number(0.0, 10.0)),
        (
            "r",
            DialValue::Range { lo: 0.0, hi: 1.0 },
            DialBounds::number(0.0, 1.0),
        ),
        (
            "a",
            DialValue::Adaptive {
                soft_min: 3.0,
                hard_max: 5.0,
            },
            DialBounds::adaptive(3.0, 5.0),
        ),
        (
            "e",
            DialValue::Enum("PROGRESSION".into()),
            DialBounds::enumerated("/Core/ItemClass", ["PROGRESSION".to_string()]),
        ),
        (
            "c",
            DialValue::Curve {
                asset: "/c.cvcurve".into(),
                row: "rate".into(),
            },
            DialBounds::default(),
        ),
        (
            "t",
            DialValue::Table {
                asset: "/c.cvcurve".into(),
                axis: "depth".into(),
            },
            DialBounds::default(),
        ),
    ];

    for (name, value, bounds) in cases {
        project.dials_mut().declare(DialMeta::authored(
            "/Content/X",
            name,
            value.clone(),
            bounds,
        ));
        let meta = project.dials().get(&format!("X.{name}")).unwrap();
        assert_eq!(meta.effective, value, "{name} did not survive");
        assert_eq!(meta.kind, value.kind());
    }
    assert_eq!(project.dials().len(), 6);
}

#[test]
fn the_editor_gets_no_private_channel() {
    // ⚠ The design forbids one, and the way to keep that true is for there to be one surface. A host
    // and a panel reach for exactly the same calls.
    let mut project = Project::new("./game.cvproj");
    project.dials_mut().declare(hookshot());

    // What a panel renders.
    let listed = project.dials().list();
    assert_eq!(listed.len(), 1);
    let meta = listed[0];
    assert_eq!(meta.kind, DialKind::Number);
    assert_eq!(meta.bounds.min, Some(8.0));
    assert!(!meta.doc.is_empty());
    assert_eq!(meta.source, DialSource::Authored);

    // What a panel's reset button does.
    project
        .dials_mut()
        .set("Hookshot.length", DialValue::Number(99.0), None)
        .unwrap();
    project.dials_mut().reset("Hookshot.length").unwrap();
    assert_eq!(
        project.dials().get("Hookshot.length").unwrap().effective,
        DialValue::Number(30.0)
    );
}
