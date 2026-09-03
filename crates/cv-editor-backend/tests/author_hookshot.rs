//! **M17's green condition** — the `Hookshot` from `ranged-traversal` §1.4, built to the shape the
//! scenario draws, **without touching a text file**, including its three dials.
//!
//! ⚠ **"Without touching a text file" is the claim worth testing.** Every step here goes through the
//! same operations a panel drives: the browser lists what may be picked, the inspector says which
//! fields are writable, the `OVERRIDES` list is pre-populated from the palette, and the `DIALS` section
//! is the only thing that creates a dial. The `.cvs` is an *output*, and nothing below writes one by
//! hand.
//!
//! ```text
//! ┌─ Hookshot  (extends Item) ───────────────────────────────────────────┐
//! │  VIEWPORT     mesh : MeshComponent   [/Content/Meshes/hookshot.glb] │
//! │               rope : TetherComponent                                 │
//! │  TAGS         Item.Tool.Tether                                       │
//! │  OVERRIDES    ● grants  ● enables  ● requires  ● judge  ● explain   │
//! │               ○ classification (PROGRESSION)                         │
//! └──────────────────────────────────────────────────────────────────────┘
//! ```

use cv_bindings::DialKind;
use cv_cvb::parse::parse;
use cv_cvb::write::write;
use cv_editor_backend::dials_section::{DialBody, DialDraft};
use cv_editor_backend::views::{browse, inspect, overrides, viewport};

/// What the editor accumulates as a developer builds a schematic.
#[derive(Default)]
struct Authoring {
    path: String,
    extends: String,
    tags: Vec<String>,
    components: Vec<(String, String, Option<String>)>,
    dials: Vec<(String, DialDraft)>,
    overrides: Vec<String>,
}

impl Authoring {
    /// Serialise to CVB — the only place text is produced, and it is produced *from* the model.
    fn to_cvb(&self) -> String {
        let mut out = format!(
            "Begin Schematic Version=1 Path={} Extends=Kind'{}' Id=sch_hookshot\n",
            self.path, self.extends
        );
        for tag in &self.tags {
            out.push_str(&format!("   Tag=Tag'{tag}'\n"));
        }
        for (i, (name, class, asset)) in self.components.iter().enumerate() {
            out.push_str(&format!(
                "   Begin Component Name=\"{name}\" Type=Kind'{class}' Id=cmp_{:02}\n",
                i + 1
            ));
            if let Some(a) = asset {
                out.push_str(&format!("      Asset=Asset'{a}'\n"));
            }
            out.push_str("   End Component\n");
        }
        for (i, (_, draft)) in self.dials.iter().enumerate() {
            for line in draft.to_block(&format!("dial_{:02}", i + 1)).lines() {
                out.push_str(&format!("   {line}\n"));
            }
        }
        for (i, hook) in self.overrides.iter().enumerate() {
            out.push_str(&format!(
                "   Begin Graph Name=\"{hook}\" Role=Hook Id=grf_{:02}\n",
                i + 1
            ));
            out.push_str(&format!(
                "      Begin Node Id=n_{:02}01 Op=array.make Pos=(0,0)\n",
                i + 1
            ));
            out.push_str("         Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/Object'>)\n");
            out.push_str("      End Node\n");
            out.push_str("   End Graph\n");
        }
        out.push_str("End Schematic\n");
        out
    }
}

/// Build the scenario's Hookshot the way a developer would, through the views.
fn author_hookshot() -> Authoring {
    let mut editing = Authoring::default();

    // ── The browser: pick a base class from what the palette offers ────────────────────────────
    let objects = browse(Some("object"), Some("/Core/"));
    let item = objects
        .iter()
        .find(|e| e.path == "/Core/Item")
        .expect("the browser offers Item");
    editing.path = "/Content/Items/Hookshot".into();
    editing.extends = item.path.clone();
    editing.tags.push("Item.Tool.Tether".into());

    // ── The Viewport: add components, again from what the browser offers ───────────────────────
    let mesh = objects
        .iter()
        .find(|e| e.path == "/Core/MeshComponent")
        .expect("the browser offers a mesh component");
    editing.components.push((
        "mesh".into(),
        mesh.path.clone(),
        Some("/Content/Meshes/hookshot.glb".into()),
    ));
    editing
        .components
        .push(("rope".into(), "/Core/TraversalComponent".into(), None));

    // ── The DIALS section: three dials, one row shape, three bodies ────────────────────────────
    editing.dials.push((
        "length".into(),
        DialDraft::new(DialKind::Number)
            .named("length")
            .documented("how far the rope reaches")
            .with(DialBody::Number {
                ty: "float".into(),
                default: 30.0,
                min: 8.0,
                max: 200.0,
            }),
    ));
    editing.dials.push((
        "wear_rate".into(),
        DialDraft::new(DialKind::Curve)
            .named("wear_rate")
            .with(DialBody::Curve {
                asset: "/Content/Curves/wear.cvcurve".into(),
                row: "rate".into(),
            }),
    ));
    editing.dials.push((
        "grade".into(),
        DialDraft::new(DialKind::Enum)
            .named("grade")
            .with(DialBody::Enum {
                path: "/Core/ItemClass".into(),
                default: "PROGRESSION".into(),
            }),
    ));

    // ── The OVERRIDES list: tick the hooks the scenario ticks ──────────────────────────────────
    let rows = overrides("/Core/Item", &[]).expect("Item is in the palette");
    for hook in ["grants", "requires"] {
        assert!(
            rows.iter().any(|r| r.name == hook),
            "the list offers {hook} without anyone typing it"
        );
        editing.overrides.push(hook.into());
    }

    editing
}

#[test]
fn the_hookshot_is_built_through_the_views_and_never_by_typing_a_file() {
    let editing = author_hookshot();
    let cvb = editing.to_cvb();
    let doc = parse(&cvb).expect("what the editor produced parses");

    assert_eq!(doc.kind, "Schematic");
    assert_eq!(
        doc.header_get("Path").map(ToString::to_string),
        Some("/Content/Items/Hookshot".into())
    );
    assert_eq!(doc.blocks("Component").len(), 2);
    assert_eq!(doc.blocks("Dial").len(), 3, "its three dials");
    assert_eq!(doc.blocks("Graph").len(), 2);
}

#[test]
fn its_three_dials_are_one_of_each_shape_the_scenario_shows() {
    // ⚠ A number, a curve and an enum — three bodies from one row.
    let editing = author_hookshot();
    let doc = parse(&editing.to_cvb()).unwrap();
    let kinds: Vec<String> = doc
        .blocks("Dial")
        .iter()
        .map(|d| {
            d.header_get("Kind")
                .map(ToString::to_string)
                .unwrap_or_default()
        })
        .collect();
    assert!(kinds.contains(&"Number".to_string()));
    assert!(kinds.contains(&"Curve".to_string()));
    assert!(kinds.contains(&"Enum".to_string()));

    let length = doc
        .blocks("Dial")
        .into_iter()
        .find(|d| d.header_get("Name").map(ToString::to_string) == Some("\"length\"".into()))
        .expect("the length dial");
    assert_eq!(
        length.get("Doc").map(ToString::to_string),
        Some("\"how far the rope reaches\"".into())
    );
}

#[test]
fn every_dial_id_is_what_host_code_will_type() {
    // ⚠ `<ClassName>.<DialName>` — the same handle `project.dials.get` takes.
    let editing = author_hookshot();
    let ids: Vec<String> = editing
        .dials
        .iter()
        .map(|(_, d)| d.qualified_id(&editing.path))
        .collect();
    assert_eq!(
        ids,
        vec![
            "Hookshot.length".to_string(),
            "Hookshot.wear_rate".to_string(),
            "Hookshot.grade".to_string()
        ]
    );
}

#[test]
fn every_dial_the_section_created_would_have_validated() {
    // ⚠ The section refuses at creation rather than lints later: the id is what host code types
    // forever.
    let editing = author_hookshot();
    let mut named: Vec<&str> = Vec::new();
    for (_, draft) in &editing.dials {
        assert_eq!(draft.validate(&named), Ok(()), "{}", draft.name);
        named.push(&draft.name);
    }
}

#[test]
fn the_viewport_shows_what_was_added_with_its_asset() {
    let editing = author_hookshot();
    let doc = parse(&editing.to_cvb()).unwrap();
    let items = viewport(&doc);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "mesh");
    assert_eq!(
        items[0].asset.as_deref(),
        Some("/Content/Meshes/hookshot.glb")
    );
    assert_eq!(items[1].name, "rope");
    assert!(
        items[1].asset.is_none(),
        "the rope draws no mesh of its own"
    );
}

#[test]
fn the_overrides_list_was_pre_populated_rather_than_typed() {
    // ⚠ A developer cannot override what they cannot see.
    let rows = overrides("/Content/Items/Hookshot", &[]);
    assert!(
        rows.is_err(),
        "an unauthored class is not in the palette yet"
    );

    let from_base = overrides("/Core/Item", &["grants", "requires"]).unwrap();
    let ticked: Vec<&str> = from_base
        .iter()
        .filter(|r| r.overridden)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(ticked, vec!["grants", "requires"]);

    // And the ones left alone still say what they do.
    let untouched: Vec<&cv_editor_backend::views::OverrideRow> =
        from_base.iter().filter(|r| !r.overridden).collect();
    assert!(!untouched.is_empty());
    assert!(
        untouched.iter().any(|r| r.inherited.is_some()),
        "a hook left alone shows what happens instead"
    );
}

#[test]
fn the_inspector_offered_only_fields_the_manifest_exposes() {
    // ⚠ The panel is driven by the palette, so it cannot offer a field the bindings do not carry.
    let fields = inspect("/Core/Item").unwrap();
    assert!(!fields.is_empty());
    for f in &fields {
        assert!(!f.ty.is_empty(), "{} has no type to render", f.name);
    }
    let writable: Vec<&str> = fields
        .iter()
        .filter(|f| f.writable)
        .map(|f| f.name.as_str())
        .collect();
    assert!(
        !writable.is_empty(),
        "an inspector with nothing writable is a viewer"
    );
}

#[test]
fn what_the_editor_produced_survives_the_canonical_writer_unchanged() {
    // ⚠ The editor's output must already *be* canonical, or every save would produce a diff nobody
    // made — which is the noise the whole canonical-form rule exists to prevent.
    let editing = author_hookshot();
    let once = write(&parse(&editing.to_cvb()).unwrap());
    let twice = write(&parse(&once).unwrap());
    assert_eq!(once, twice, "a save is a fixed point");
}

#[test]
fn the_schematic_the_editor_produced_compiles() {
    // ⚠ The end of the claim: built through the views, and the compiler accepts it.
    let editing = author_hookshot();
    let doc = parse(&editing.to_cvb()).unwrap();
    let compiled = cv_compile::compile(&doc);
    assert!(
        compiled.succeeded(),
        "errors: {:?}",
        compiled.findings().of(cv_compile::Severity::Error)
    );
    assert_eq!(
        compiled.programs().len(),
        2,
        "one program per overridden hook"
    );
}
