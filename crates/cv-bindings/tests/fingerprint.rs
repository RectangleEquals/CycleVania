//! **The fingerprint is the recipe** — `H(core version, content digests, config)`.
//!
//! ⚠ **It hashed the project's file path and its dials, and nothing else.** Editing a schematic left it
//! unchanged, so *"same fingerprint + same seed ⇒ same world"* — the contract the whole determinism
//! story rests on — was false: two projects with entirely different content reported as the same world.
//! And moving a project **did** change it, which the plan forbids outright, because *a move is not a
//! content change* and a fingerprint that disagreed would stop every reproduction bundle reproducing.
//!
//! ▶ **`cv-core` had the right implementation the whole time.** `FingerprintBuilder` folds in the core
//! version, content and config; the binding rolled its own two-line hash beside it and nothing compared
//! them. These tests are the comparison.

use cv_bindings::project::Project;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("a scratch directory");
    base
}

const V1: &str = "Begin X Id=x Version=1\n   A=1\nEnd X\n";
const V2: &str = "Begin X Id=x Version=2\n   A=99\nEnd X\n";

#[test]
fn adding_content_changes_the_recipe() {
    let at = scratch("cv-fp-add").join("game.cvproj");
    let mut p = Project::create(&at, None).expect("create");
    let empty = p.fingerprint();
    p.write("a.cvs", V1).expect("write");
    assert_ne!(
        empty,
        p.fingerprint(),
        "a file that was not there is a different recipe"
    );
}

#[test]
fn editing_content_changes_the_recipe() {
    // ⚠ **The one that was false.** This is the whole defect in a single assertion.
    let at = scratch("cv-fp-edit").join("game.cvproj");
    let mut p = Project::create(&at, None).expect("create");
    p.write("a.cvs", V1).expect("write");
    let before = p.fingerprint();
    p.write("a.cvs", V2).expect("edit");
    assert_ne!(
        before,
        p.fingerprint(),
        "editing a schematic must change the recipe — otherwise two different worlds claim to be one"
    );
}

#[test]
fn moving_a_project_does_not_change_the_recipe() {
    // ⚠ **A move is not a content change.** The old fingerprint hashed the path, so it disagreed —
    // and a fingerprint that disagrees on a move stops every reproduction bundle reproducing.
    let from = scratch("cv-fp-move-a");
    let at = from.join("game.cvproj");
    let mut p = Project::create(&at, None).expect("create");
    p.write("a.cvs", V1).expect("write");
    let original = p.fingerprint();

    let to = scratch("cv-fp-move-b");
    std::fs::create_dir_all(to.join("content")).expect("content root");
    std::fs::copy(&at, to.join("game.cvproj")).expect("copy descriptor");
    std::fs::copy(from.join("content/a.cvs"), to.join("content/a.cvs")).expect("copy content");

    let moved = Project::open(to.join("game.cvproj")).expect("open the moved project");
    assert_eq!(
        original,
        moved.fingerprint(),
        "the same content in a different place is the same recipe"
    );
}

#[test]
fn a_dial_is_part_of_the_recipe_and_a_seed_is_not() {
    // ⚠ The asymmetry both concepts exist for: a changed dial is a different recipe, a changed seed is
    // the same recipe rolled again.
    let at = scratch("cv-fp-seed").join("game.cvproj");
    let mut p = Project::create(&at, None).expect("create");
    p.write("a.cvs", V1).expect("write");
    p.validate().expect("validate");

    let one = p
        .generate(cv_bindings::project::GenerateOptions::seeded("alpha"))
        .expect("generate");
    let two = p
        .generate(cv_bindings::project::GenerateOptions::seeded("beta"))
        .expect("generate");
    assert_eq!(
        one.fingerprint, two.fingerprint,
        "a seed does not change the recipe"
    );
    assert_ne!(one.seed, two.seed);
}

#[test]
fn the_core_version_is_an_input() {
    // ⚠ **The question that started this.** The version displayed everywhere was `0.1.0` against a
    // v0.2b design, and it was *supposed* to be part of the fingerprint — `Descriptor.cyclevania` says
    // so — while nothing folded it in. `FingerprintBuilder::for_this_build()` starts from it, so a
    // rebuilt engine can never claim to reproduce a world an older one made.
    let at = scratch("cv-fp-version").join("game.cvproj");
    let p = Project::create(&at, None).expect("create");

    let mine = p.fingerprint();
    let same = cv_core::fingerprint::FingerprintBuilder::for_this_build()
        .config_f64("world_scale", 1.0)
        .finish()
        .to_raw();
    assert_eq!(
        mine, same,
        "an empty project is the core version and its scale, and nothing else"
    );

    let older = cv_core::fingerprint::FingerprintBuilder::new("0.1.0")
        .config_f64("world_scale", 1.0)
        .finish()
        .to_raw();
    assert_ne!(
        mine, older,
        "a different core version is a different recipe"
    );
}

#[test]
fn a_cooked_project_still_answers() {
    // A package has no content root to walk; it must still produce a recipe id rather than panic.
    let cooked = Project::load_from_file("game.cvpak");
    assert_ne!(cooked.fingerprint(), 0);
    assert!(
        !Path::new("game.cvpak").exists(),
        "and without needing the file to exist"
    );
}
