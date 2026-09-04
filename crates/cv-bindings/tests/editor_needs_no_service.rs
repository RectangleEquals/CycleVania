//! **M15b's green condition, and M16's standing guard.**
//!
//! ⚠ **The editor must never need help from anything but the bindings.** The moment it cannot do
//! something in-process, the pressure is to put a service behind it — and that is exactly how a whole
//! editor subsystem once ended up in Rust, five milestones before anyone noticed.
//!
//! ▶ **So this test asks the question in the only cheap moment there is**: before the editor exists. A
//! hole found here is a binding to add; the same hole found at M16 is an architecture to argue about.
//!
//! # Two halves, because either alone is fooled
//!
//! | | Catches |
//! |---|---|
//! | the list of needs | a method that was never written |
//! | the end-to-end walk | ⚠ methods that all exist and **do not compose** — open, write, read back, and get something else |

use cv_bindings::project::Project;
use std::path::PathBuf;

/// Everything [`10-editor.md`] §1.3 and the plan's M16 require of the seam.
///
/// ⚠ **Named here rather than inferred**, so adding an editor need means adding a row and watching it
/// fail — not discovering at M16 that the surface was decided by whatever happened to get written.
const THE_EDITOR_NEEDS: &[(&str, &str)] = &[
    ("open", "M16 P02 — open a project"),
    ("content", "M16 P02 — list what is in it"),
    ("read", "M16 P02 — read a content file"),
    ("write", "M16 P04 — save one, byte-identically"),
    ("validate", "M16 P02 — check before generating"),
    ("generate", "M16 P02 — generate a world"),
    ("dials", "M19 — the Dials view is `project.dials`"),
    (
        "create",
        "M16 P04a — a new project, which only the editor can make",
    ),
    (
        "load_from_file",
        "the cooked-build entry point a host shares",
    ),
    (
        "may_paste",
        "M18 P06 — whether a fragment may paste here; the format rule is the core's",
    ),
];

fn surface() -> String {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    std::fs::read_to_string(src).expect("the binding surface")
}

#[test]
fn the_handle_carries_everything_the_editor_needs() {
    let src = surface();
    let missing: Vec<&str> = THE_EDITOR_NEEDS
        .iter()
        .filter(|(name, _)| !src.contains(&format!("pub fn {name}(")))
        .map(|(_, why)| *why)
        .collect();
    assert!(
        missing.is_empty(),
        "the binding does not carry: {missing:#?}\n  \
         ⚠ Every one of these is a reason the editor would need a service. Add the binding."
    );
}

#[test]
fn both_targets_carry_it_not_just_the_one_that_happened_to_be_built() {
    // ⚠ **A surface only one target has is not a surface.** napi is the default feature, so a
    // method added to it alone compiles, tests green, and is missing from every WASM host.
    let src = surface();
    let napi = src.find("js_name = \"Project\"").expect("a napi handle");
    let wasm = src.rfind("js_name = \"Project\"").expect("a wasm handle");
    assert_ne!(napi, wasm, "only one target declares the handle");
    // ⚠ Split at the WASM handle: everything before is the napi half, everything after the web half.
    // A free function like `may_paste` is declared once per target too, so the same check covers it.
    let (native, web) = src.split_at(wasm);
    for (name, why) in THE_EDITOR_NEEDS {
        let decl = format!("pub fn {name}(");
        assert!(native.contains(&decl), "napi is missing {name} — {why}");
        assert!(web.contains(&decl), "wasm is missing {name} — {why}");
    }
}

#[test]
fn a_project_can_be_created_written_read_back_and_generated_through_the_seam() {
    // ⚠ **The half a list of names cannot check.** Nine methods that each exist and do not compose
    // would pass the test above and leave the editor unable to do a single useful thing.
    let base = std::env::temp_dir().join("cv-m15b-walk");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("a scratch directory");
    let at = base.join("game.cvproj");

    let mut project = Project::create(&at, None).expect("create from nothing");
    assert!(project.content().is_empty(), "a new project starts empty");

    let authored = "Begin X Id=x Version=1\n   B=2\n   A=1\nEnd X\n";
    let written = project.write("thing.cvs", authored).expect("write");
    assert_eq!(
        project.read("thing.cvs").expect("read back"),
        written,
        "what comes back must be what went in"
    );
    assert_eq!(
        project.content(),
        vec!["thing.cvs"],
        "a written file is a listed file"
    );

    // ⚠ Writing invalidates: the tree changed, so the last validate describes something that is gone.
    assert!(
        project
            .generate(cv_bindings::project::GenerateOptions::seeded("s"))
            .is_err(),
        "generate must refuse until validate has run again"
    );
    project.validate().expect("validate");
    let world = project
        .generate(cv_bindings::project::GenerateOptions::seeded("world-42"))
        .expect("generate");
    assert_eq!(world.seed, "world-42");

    // ⚠ Re-opening must see the same thing — otherwise "save" meant "hold in memory".
    let reopened = Project::open(&at).expect("reopen");
    assert_eq!(reopened.content(), vec!["thing.cvs"]);
    assert_eq!(reopened.read("thing.cvs").unwrap(), written);
}

#[test]
fn creating_from_a_project_copies_its_content_rather_than_linking_it() {
    // ⚠ **A preset is a real host project, which is what makes it usable as a template** — and what
    // makes sharing files with it wrong: presets are also the acceptance tests, so they change.
    let base = std::env::temp_dir().join("cv-m15b-copy");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("a scratch directory");

    let source_path = base.join("preset.cvproj");
    let mut source = Project::create(&source_path, None).expect("the preset");
    source
        .write("a.cvs", "Begin X Id=x Version=1\nEnd X\n")
        .expect("author it");

    let made_path = base.join("made/game.cvproj");
    let source = Project::open(&source_path).expect("reopen the preset");
    let made = Project::create(&made_path, source.descriptor()).expect("create from it");
    assert_eq!(made.content(), vec!["a.cvs"], "content came across");

    // Changing the new project must not touch the one it came from.
    let mut made = made;
    made.write("a.cvs", "Begin X Id=x Version=2\nEnd X\n")
        .expect("edit the copy");
    assert!(
        source.read("a.cvs").unwrap().contains("Version=1"),
        "the preset was modified through the copy — it was linked, not copied"
    );
}

#[test]
fn a_cooked_build_says_so_rather_than_pretending_to_have_no_files() {
    // ⚠ **"No content root" is not "no files".** A cooked package carries its content inside itself;
    // an empty list would read as an empty project, which is a different and wrong answer.
    let cooked = Project::load_from_file("game.cvpak");
    assert!(cooked.read("anything.cvs").is_err());
    assert!(cooked.descriptor().is_none());
}
