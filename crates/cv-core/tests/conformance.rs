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

/// Every **committed text file**, not just this crate's, and not just Rust.
///
/// ⚠ **Scanning one crate is how `cv-vm` and `cv-determinism` drifted unchecked.** Both carried
/// claims about a text scripting language and a shipped bytecode artifact, neither of which exists, and
/// no lint looked at them because the lint lived next to the crate it was written for.
///
/// ⚠ **Scanning only Rust is how `README.md` did the same, for longer.** It advertised an `L0-L6`
/// pipeline for nineteen milestones — and so did `cv-core`'s crates.io `description`, which is
/// *published metadata* — while a lint banning that exact string ran green two directories away. A
/// checker that reads one file type teaches everyone that the other file types are not checked.
///
/// ▶ **`git ls-files` is the boundary, because "committed" is the actual question.** A directory
/// walk would either miss a new top-level file or wander into the private notes beside the repo; the
/// index knows exactly what is public.
/// Is this a generated dependency lockfile?
fn is_lockfile(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "Cargo.lock" || n.ends_with("-lock.json") || n == "pnpm-lock.yaml")
}

fn committed_text() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::process::Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(&root)
        .output()
        .expect("git is required to know what is committed");
    let listed: Vec<PathBuf> = String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|rel| root.join(rel))
        .filter(|p| {
            // ⚠ **Lockfiles are skipped, and not because they are inconvenient.** They are generated
            // dependency manifests full of base64 integrity hashes, so any short token appears in them
            // by chance — `package-lock.json` produced two `L6` hits inside SHA-512 digests. There is
            // no prose in a lockfile to be wrong about.
            !is_lockfile(p)
                && p.extension().is_some_and(|x| {
                    matches!(
                        x.to_str(),
                        Some("rs" | "toml" | "md" | "json" | "ts" | "js" | "yml" | "yaml")
                    )
                })
        })
        .collect();
    assert!(
        !listed.is_empty(),
        "listed no committed files — a lint that checks nothing passes every time"
    );
    listed
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
        "CVB file",
        "CVB is a notation, not a format; the file is a schematic, a spine template or a state graph",
    ),
    (
        "scheduling layer",
        "03-pipeline.md §1 states there is no scheduling layer — schedules are arbitrated inside L1",
    ),
];

#[test]
fn no_superseded_concept_survives_anywhere_committed() {
    let mut offenders = Vec::new();
    for path in committed_text() {
        let Ok(src) = fs::read_to_string(&path) else {
            continue;
        };
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        // This file names them on purpose — it is the table.
        if name == "conformance.rs" {
            continue;
        }
        // ⚠ **In Rust, only comments; everywhere else, every line.** A `.md` or a `.toml` has no
        // comment syntax to hide behind — the whole file is the claim. Restricting the scan to `//` in
        // prose would reproduce the exact blind spot this test was widened to close.
        let rust = path.extension().is_some_and(|x| x == "rs");
        // ⚠ Collected once. `nth()` per line is O(n²), and `cv-api/src/lib.rs` is 6,200 lines
        // — enough to turn a lint into something people notice and then start skipping.
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            if rust && !(t.starts_with("//") || t.starts_with("///") || t.starts_with("//!")) {
                continue;
            }
            // ⚠ **A denial is not a use, and neither is a history.** *"There is no scheduling
            // layer"* is the sentence that documents the rule; *"v0.1 numbered the pipeline L0-L6"*
            // explains why a reader will not find one. A checker that cannot tell either from an
            // affirmative use flags the very comments it wants written — failing in the direction
            // that looks like diligence, which is how a lint trains people to ignore it.
            //
            // The signal is a marker of pastness or absence. Every note here that legitimately names
            // a superseded thing carries one, because that is how a supersession gets explained.
            const HISTORICAL: &[&str] = &[
                "there is no",
                "v0.1",
                "pre-v0.2",
                "used to",
                "no longer",
                "superseded",
                "never ships",
                "not committed",
                "was deleted",
                "repurposed",
                "not a declared",
                "must not",
                "deliberately not",
                "does not exist",
            ];
            // ⚠ Compared with emphasis stripped and case folded. Chasing `**Deliberately not**` versus
            // `deliberately **not**` is a game the lint loses: a marker that depends on where an author
            // ⚠ **Compared with emphasis stripped and case folded.** Chasing `**Deliberately not**`
            // versus `deliberately **not**` is a game the lint loses: a marker that depends on where an
            // author put an asterisk is a marker that fails on correct prose.
            //
            // ⚠ **And with the previous line, because prose wraps.** `README.md` said *"There is no"* at
            // the end of one line and *"scheduling layer"* at the start of the next — a denial the
            // line-based check could not see, on the very sentence that documents the rule. A lint that
            // flags correct writing for where the author's editor broke the line is a lint people
            // silence.
            let context = format!("{} {line}", i.checked_sub(1).map_or("", |q| lines[q]));
            let plain = context.replace(['*', '_', '`'], "").to_lowercase();
            if HISTORICAL.iter().any(|d| plain.contains(d)) {
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
        "committed files describe concepts v0.2+ superseded:
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
