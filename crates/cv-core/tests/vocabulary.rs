//! The vocabulary lint.
//!
//! `.notes/Design/v0.2b` renamed three things the code had called by older names. A rename is only
//! worth doing once, so this test is the thing that keeps it done: it fails on the old stems rather
//! than trusting everyone to remember.
//!
//! # Why a test and not a review habit
//!
//! Most of these are *near-synonyms of a surviving word*. `Reach` is a scope and stays; `reachable`
//! is drift and goes. A *lexer* token stays; `ContentKind::Token` goes. A reviewer reading one diff
//! hunk cannot reliably tell which sense is in front of them, and the wrong one reads fine. A
//! compiler cannot help either, because both spellings are valid identifiers.
//!
//! # The renames
//!
//! | Old | New | Why |
//! |---|---|---|
//! | `Capability` → `Token` | **`Unlock`** | ⚠ renamed **twice**. M03 made it `Token`; M03a made it an `Unlock` **row** of an `UnlockTableResource`, because it was a class with no behaviour — data wearing a class's clothes |
//! | `reachable` / `reachability` | `accessible` / `accessibility` | `Reach` is a scope; the adjective was colliding with it |
//! | `Biome`, `Motif` | no type at all | both are *patterns* — dial values on an Area-scoped spine slot |
//!
//! # ⚠ Symbols, not prose
//!
//! The rule enforced here is the one `07-naming.md` §5 states:
//!
//! > **The concept keeps its name in prose; no *symbol* uses the stem unless it means the scope.**
//!
//! *"Reachability analysis"* is the domain term for the mathematics and is explicitly **kept** in
//! prose; `struct Reachability` is a symbol and is not. Likewise a *"text token in a file"* is a lexer
//! token and stays, while `ContentKind::Token` goes. So this lint reads **code only** — a stem inside
//! a `//` comment is left alone, deliberately.
//!
//! ⚠ M03 got this wrong in the other direction: it rewrote prose too, which broke grammar
//! (*"a accessibility sweep"*) and would have rejected the exact wording the design mandates.
//!
//! ⚠ **`reach` itself is not banned.** `Reach` the scope, `reaches` as a plural of it, and `ctx.reach`
//! are all correct and common. Only the adjective forms were ever drift, which is exactly why this
//! lint matches stems rather than the bare word.

use std::fs;
use std::path::{Path, PathBuf};

/// A banned stem, and what to write instead.
const BANNED: &[(&str, &str)] = &[
    ("Capabilit", "an `Unlock` — a row of an UnlockTableResource"),
    (
        "capabilit",
        "an unlock — the English word is fine in prose, a symbol is not",
    ),
    ("reachable", "accessible"),
    ("Reachable", "Accessible"),
    ("reachabilit", "accessibilit"),
    ("Reachabilit", "Accessibilit"),
    (
        "ContentKind::Biome",
        "nothing — a biome is dial values on an Area-scoped spine slot",
    ),
    (
        "ContentKind::Motif",
        "nothing — a motif is a chain the solver invents",
    ),
    // M03a: the lattice atom is an `Unlock` row, not a token class.
    (
        "Token",
        "`Unlock` — a row of an UnlockTableResource, not a class",
    ),
    (
        "token",
        "an unlock — the lexer sense is fine in prose, a symbol is not",
    ),
];

/// Strip what a banned stem is allowed to hide inside.
///
/// Three exemptions, all deliberate:
/// * `unreachable!` is a std macro, not our vocabulary.
/// * everything from `//` onward is **prose**, which the naming rule explicitly leaves alone.
/// * ⚠ **`session_token` is the third sense of the word**, alongside the progression one this lint
///   bans and the lexer one its header already exempts. The design mandates it — *"LAN auth and session
///   tokens"* — so a lint that refused it would reject the exact wording the design asks for, which is
///   the failure this file's own header warns M03 made in the other direction. **Only the compound is
///   exempt**: bare `token` stays banned, so every use site says which sense it means, which is worth
///   more here than in a codebase where the progression sense had never existed.
fn strip_allowed(line: &str) -> String {
    let line = line.replace("session_token", "«session»");
    let line = line.as_str();
    let code = match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    };
    code.replace("unreachable!", "").replace("unreachable", "")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_old_vocabulary_is_gone_and_stays_gone() {
    // Walk the whole workspace, not just this crate: the rename was tree-wide and a reintroduction
    // is just as wrong in cv-geometry as it is here.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    assert!(
        files.len() > 10,
        "found only {} files — the walk is wrong",
        files.len()
    );

    let mut offenders = Vec::new();
    for path in &files {
        // Generated from the manifest; fixing it here would be fixing the wrong file.
        if path.ends_with("cv-api/src/lib.rs") || path.ends_with("vocabulary.rs") {
            continue;
        }
        let Ok(src) = fs::read_to_string(path) else {
            continue;
        };
        for (i, line) in src.lines().enumerate() {
            let scrubbed = strip_allowed(line);
            for (stem, fix) in BANNED {
                if scrubbed.contains(stem) {
                    let rel = path.strip_prefix(&root).unwrap_or(path);
                    offenders.push(format!(
                        "{}:{}: `{stem}` — write {fix}\n      {}",
                        rel.display(),
                        i + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the v0.2b vocabulary migration (M03) regressed in {} place(s):\n  {}",
        offenders.len(),
        offenders.join("\n  ")
    );
}

/// The manifest is the authored surface; drift there propagates into four generated artifacts.
#[test]
fn the_manifest_uses_the_current_vocabulary() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../manifest/tier1.toml");
    let src = fs::read_to_string(&manifest).expect("the manifest is readable");

    let mut offenders = Vec::new();
    for (i, line) in src.lines().enumerate() {
        // A manifest line is `key = value` or a doc string. Only keys and type names are symbols;
        // `doc = "…"` is prose and plays by the prose rule.
        let symbols = match line.split_once("doc = ") {
            Some((before, _)) => before,
            None => line,
        };
        for (stem, fix) in BANNED {
            // The manifest declares no Rust paths, so the ContentKind entries cannot appear.
            if stem.starts_with("ContentKind") {
                continue;
            }
            if symbols.contains(stem) {
                offenders.push(format!("tier1.toml:{}: `{stem}` — write {fix}", i + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the manifest carries old vocabulary, which would reach the generated bindings, palette and \
         docs:\n  {}",
        offenders.join("\n  ")
    );
}
