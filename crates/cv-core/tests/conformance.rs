//! The conformance lint — the design is the enumerable set, not the tree.
//!
//! # What went wrong, and why a test is the fix
//!
//! The implementation plan's second section used to be a module-by-module walk of the codebase
//! pronouncing each file **"survives"**. That is a claim about *design conformance* reached by
//! *reading code*, and it was wrong wherever it was checkable — it blessed `node.rs` while `NodeKind`
//! was missing the `Floor` scope, called `spine.rs` *"survives intact"* when it had neither fill bands
//! nor parallel groups, and called `solver.rs` *"survives"* when it was built on `progression_locality`,
//! a dial the design **refuses**.
//!
//! ⚠ The failure mode is silence. Nothing breaks when the core carries a concept the design dropped —
//! it compiles, the tests pass, and it reaches the gate. `Linearity` sat in `Solver`'s constructor
//! across four milestones for exactly that reason.
//!
//! > **The source-of-truth order is `v0.2b > v0.2 > v0.1 > pre-v0.1 > the codebase`.** The codebase is
//! > the *output* of the plan, never an input to it.
//!
//! # What this checks
//!
//! Two things, both cheap and both the kind nobody catches by eye:
//!
//! 1. **No concept the design explicitly refuses** appears anywhere in the core.
//! 2. **No code comment cites a milestone number.** The numbers collide across plans — `(M09)` is the
//!    mission graph in v0.1 and dials in v0.2b — so a citation in code points a reader at the wrong
//!    document with no way to tell.
//!
//! ⚠ It deliberately does **not** require every public type to appear in the manifest. The manifest
//! declares the *authored API surface*; `Arena`, `Handle` and `Writer` are Rust machinery implementing
//! design behaviour, and demanding a declaration for them would force junk into tier-1. Distinguishing
//! the two needs judgement, which is what the plan's conformance matrix is for.

use std::fs;
use std::path::{Path, PathBuf};

/// A concept the design refuses, and where it says so.
///
/// ⚠ Everything here was **found in the tree**, not imagined. Each entry is a thing the codebase
/// actually carried while the design said it should not exist.
const REFUSED: &[(&str, &str)] = &[
    (
        "progression_locality",
        "05-object-model.md §4.2: \"There is deliberately no progression_locality dial\" — a door \
         states key-to-lock distance as a MinDistanceFrom constraint",
    ),
    (
        "LinearityResolver",
        "the pre-v0.1 dial resolver; deleted at M04a with the dials it resolved",
    ),
    (
        "FlowKind",
        "05-object-model.md §2: \"an enum, a string and a dictionary wearing a class's clothes\" — \
         authored Interaction subclasses replace it",
    ),
    (
        "MechanicRegistry",
        "the pre-script seam, deleted at M04; dispatch returns at M13 through the VM",
    ),
];

fn core_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    walk(&root.join("src"), &mut out);
    out
}

/// Every Rust source in the workspace, not just this crate's.
///
/// ⚠ **Scanning one crate is how `cv-vm` and `cv-determinism` drifted unchecked.** Both carried claims
/// about a text scripting language and a `.cvb` artifact that cannot exist, and no lint looked
/// at them because the lint lived next to the crate it was written for.
fn workspace_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    walk(&root.join("crates"), &mut out);
    out
}

/// A concept that v0.2 or v0.2b **superseded**, and what it is now.
///
/// ⚠ **A stale comment is worse than no comment.** It reads as current, so the next person to touch the
/// file builds on it — and `geometry.rs` carried a runnable-looking example calling two deleted APIs.
/// The v0.1 pipeline is the sharpest case: v0.2 folded scheduling into the content layer and
/// **renumbered everything**, so a comment saying `L2` may be wrong even where `L2` still exists.
const SUPERSEDED: &[(&str, &str)] = &[
    (
        "L0-L6",
        "v0.2 folded the scheduling layer into L0; the pipeline is L0-L5          (Content, Mission, Skeleton, Volume, Geometry, Finalize)",
    ),
    (
        "L0–L6",
        "v0.2 folded the scheduling layer into L0; the pipeline is L0-L5",
    ),
    ("L6", "there is no L6; v0.1's dressing layer is v0.2b's L5 Finalize"),
    (
        ".cvb",
        "CVB is a NOTATION, not a file type — `.cvs`, `.cvspine` and `.cvstate` are separate          formats written in it, so an extension named after the notation is a category error",
    ),
    (
        "CVB file",
        "CVB is a notation, not a format; the file is a schematic, a spine template or a state graph",
    ),
    (
        "scheduling layer",
        "03-pipeline.md §1 states there is no scheduling layer — schedules are arbitrated inside L1",
    ),
];

#[test]
fn no_superseded_concept_survives_in_a_comment() {
    let mut offenders = Vec::new();
    for path in workspace_sources() {
        let Ok(src) = fs::read_to_string(&path) else {
            continue;
        };
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        // This file names them on purpose — it is the table.
        if name == "conformance.rs" {
            continue;
        }
        for (i, line) in src.lines().enumerate() {
            let t = line.trim();
            if !(t.starts_with("//") || t.starts_with("///") || t.starts_with("//!")) {
                continue;
            }
            // ⚠ **A denial is not a use.** *"There is no scheduling layer"* is the sentence that
            // documents the rule, and a checker that cannot tell it from an affirmative use flags the
            // very comment it wants written — failing in the direction that looks like diligence,
            // which is how a lint trains people to ignore it.
            let denies = [
                "there is no",
                "There is no",
                "no scheduling layer",
                "not a declared",
            ]
            .iter()
            .any(|d| line.contains(d));
            if denies {
                continue;
            }
            for (term, why) in SUPERSEDED {
                if line.contains(term) {
                    offenders.push(format!("{name}:{}: `{term}` — {why}", i + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "comments describe concepts v0.2+ superseded:
  {}",
        offenders.join(
            "
  "
        )
    );
}

#[test]
fn the_core_carries_nothing_the_design_refuses() {
    let mut offenders = Vec::new();
    for path in core_sources() {
        let Ok(src) = fs::read_to_string(&path) else {
            continue;
        };
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        for (i, line) in src.lines().enumerate() {
            // Prose explaining why something is absent is not the thing being absent.
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            for (concept, why) in REFUSED {
                if line.contains(concept) {
                    offenders.push(format!("{name}:{}: `{concept}` — {why}", i + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the core carries {} concept(s) the design refuses:\n  {}\n\n\
         If the design changed, change the design first and update REFUSED. If it did not, this is \
         pre-v0.2 code that survived a migration.",
        offenders.len(),
        offenders.join("\n  ")
    );
}

#[test]
fn no_comment_cites_a_milestone_number() {
    // ⚠ 34 of these existed before M04a, all citing the *v0.1* plan. `lib.rs` even described two
    // modules that had been deleted. A number in a comment cannot say which plan it means.
    let pattern = regex_lite();
    let mut offenders = Vec::new();
    for path in core_sources() {
        let Ok(src) = fs::read_to_string(&path) else {
            continue;
        };
        // This file names milestones on purpose — it is about them.
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        for (i, line) in src.lines().enumerate() {
            if let Some(m) = pattern(line) {
                offenders.push(format!(
                    "{name}:{}: `{m}` — say what it is, not when",
                    i + 1
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "milestone numbers in code comments cite a plan the reader cannot identify:\n  {}",
        offenders.join("\n  ")
    );
}

/// Finds `(M12)` / `(M10a)` without pulling in a regex dependency.
///
/// Hand-rolled because `cv-core` has **no dependencies outside the workspace** and a lint is not a
/// reason to acquire one.
fn regex_lite() -> impl Fn(&str) -> Option<String> {
    |line: &str| {
        let b = line.as_bytes();
        let mut i = 0;
        while i + 3 < b.len() {
            if b[i] == b'(' && b[i + 1] == b'M' && b[i + 2].is_ascii_digit() {
                let mut j = i + 2;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                if j < b.len() && b[j].is_ascii_lowercase() {
                    j += 1;
                }
                if j < b.len() && b[j] == b')' {
                    return Some(line[i..=j].to_string());
                }
            }
            i += 1;
        }
        None
    }
}
