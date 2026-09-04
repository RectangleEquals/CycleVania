//! **`validate` compiles the content, and this proves it.**
//!
//! ⚠ **It was a stub for four milestones and nothing recorded that.** It checked a findings list only a
//! test could populate, so a project full of broken schematics validated cleanly and `generate` went
//! ahead on it. ▶ **A check that cannot fail is worse than no check** — it answers the question
//! everybody then stops asking.
//!
//! ⚠ **So the test that matters here is the failing one.** A green `validate` proves nothing on its own;
//! it proved nothing for four milestones.

use cv_bindings::project::Project;
use std::path::PathBuf;

/// A schematic with one graph, shaped like `cv-compile`'s own fixtures.
fn schematic(body: &str) -> String {
    format!(
        "Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=s\n\
         {body}End Schematic\n"
    )
}

fn clean() -> String {
    schematic(
        "   Begin Graph Name=\"requires\" Role=Hook Id=grf\n      \
         Begin Node Id=n_0001 Op=core.instances_of Pos=(0,0)\n         \
         Pin (Name=out, Dir=Out, Type=bool, To=(n_0002.cond))\n      End Node\n      \
         Begin Node Id=n_0002 Op=core.branch Pos=(80,0)\n      End Node\n   End Graph\n",
    )
}

fn broken() -> String {
    // ⚠ A typo'd op — the compiler names the node and suggests the real one.
    schematic(
        "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
         Begin Node Id=n_0009 Op=array.is_emty Pos=(0,0)\n      End Node\n   End Graph\n",
    )
}

fn scratch(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("a scratch directory");
    base.join("game.cvproj")
}

#[test]
fn a_broken_schematic_fails_validation_and_says_which_file() {
    let at = scratch("cv-validate-broken");
    let mut project = Project::create(&at, None).expect("create");
    project.write("hook.cvs", &broken()).expect("write");

    let err = project
        .validate()
        .expect_err("a schematic with an unknown op must not validate");
    let text = err.to_string();
    assert!(
        text.contains("hook.cvs"),
        "the finding must name the file: {text}"
    );
    assert!(text.contains("no op named"), "and what is wrong: {text}");

    // ⚠ And generate must still refuse, because validate did not pass.
    assert!(project
        .generate(cv_bindings::project::GenerateOptions::seeded("s"))
        .is_err());
}

#[test]
fn a_clean_schematic_validates_and_then_generates() {
    let at = scratch("cv-validate-clean");
    let mut project = Project::create(&at, None).expect("create");
    project.write("hook.cvs", &clean()).expect("write");

    project.validate().expect("a clean schematic validates");
    project
        .generate(cv_bindings::project::GenerateOptions::seeded("world-42"))
        .expect("and then generates");
}

#[test]
fn fixing_the_file_clears_the_finding() {
    // ⚠ **The half that a failing test alone does not cover.** A validate that always failed would pass
    // the first test here and be just as useless as one that always passed.
    let at = scratch("cv-validate-fix");
    let mut project = Project::create(&at, None).expect("create");
    project.write("hook.cvs", &broken()).expect("write");
    assert!(project.validate().is_err());

    project.write("hook.cvs", &clean()).expect("fix it");
    project
        .validate()
        .expect("the finding is gone once the file is");
}

#[test]
fn only_schematics_are_compiled() {
    // ⚠ Curves, unlock tables and tags are data the loader checks its own way. Handing them to a graph
    // compiler would report "not a schematic" against every one of them — noise standing exactly where
    // a real finding should be.
    let at = scratch("cv-validate-formats");
    let mut project = Project::create(&at, None).expect("create");
    project
        .write("hook.cvs", &clean())
        .expect("write the schematic");
    project
        .write("notes.cvtags", "Begin X Id=x Version=1\nEnd X\n")
        .expect("write a non-schematic");

    project
        .validate()
        .expect("a non-schematic must not be compiled as one");
}
