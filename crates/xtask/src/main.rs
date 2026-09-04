//! Build-time tooling.
//!
//! **M01: the manifest generator.** `manifest/tier1.toml` is the only place a tier-1 signature is
//! written by hand. This binary turns it into every downstream artifact:
//!
//! | Artifact | Consumer |
//! |---|---|
//! | `crates/cv-api/src/lib.rs` | the core — a descriptor table it dispatches and validates through |
//! | `crates/cv-bindings/index.d.ts` | TypeScript hosts |
//! | `editor/palette.json` | the editor's node palette (M18) |
//! | `docs/authoring/api-reference.md` | developers |
//!
//! ```text
//! cargo xtask generate    # write them
//! cargo xtask check       # regenerate in memory and fail on any difference
//! ```
//!
//! `check` is what CI runs. It is the mechanism that makes "edit the manifest, never the output" a
//! rule the build enforces rather than a convention people remember.

mod emit_docs;
mod emit_palette;
mod emit_rust;
mod emit_ts;

use cv_manifest::{parse, validate, Manifest};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// One generated file: where it goes, and what should be in it.
struct Artifact {
    path: &'static str,
    body: String,
}

fn main() -> ExitCode {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "generate" => run(false),
        "check" => run(true),
        other => {
            eprintln!("unknown task `{other}`\n\nusage: cargo xtask <generate|check>");
            ExitCode::from(2)
        }
    }
}

fn run(check_only: bool) -> ExitCode {
    let root = repo_root();
    let manifest_path = root.join(cv_manifest::DEFAULT_PATH);

    let src = match std::fs::read_to_string(&manifest_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {e}", manifest_path.display());
            return ExitCode::FAILURE;
        }
    };

    let m = match parse(&src) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("the manifest does not parse\n  {e}");
            return ExitCode::FAILURE;
        }
    };

    // Refuse to generate from an illegal manifest. Generating anyway would spread one bad
    // declaration across four files and make the real error harder to find, not easier.
    let violations = validate(&m);
    if !violations.is_empty() {
        eprintln!("the manifest violates its own constraints, so nothing was generated:");
        for v in &violations {
            eprintln!("  {v}");
        }
        return ExitCode::FAILURE;
    }

    let artifacts = build(&m);

    let mut stale = Vec::new();
    for a in &artifacts {
        let path = root.join(a.path);
        let current = std::fs::read_to_string(&path).ok();
        if current.as_deref() == Some(a.body.as_str()) {
            continue;
        }
        if check_only {
            stale.push(a.path);
            continue;
        }
        if let Some(dir) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                eprintln!("cannot create {}: {e}", dir.display());
                return ExitCode::FAILURE;
            }
        }
        if let Err(e) = std::fs::write(&path, &a.body) {
            eprintln!("cannot write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!("wrote {}", a.path);
    }

    if check_only && !stale.is_empty() {
        eprintln!("generated artifacts are stale:");
        for p in &stale {
            eprintln!("  {p}");
        }
        eprintln!("\nrun `cargo xtask generate` and commit the result.");
        eprintln!("if you edited one of these by hand, that edit is about to be lost — put it in");
        eprintln!("manifest/tier1.toml instead, which is the only file that is authored.");
        return ExitCode::FAILURE;
    }

    if check_only {
        println!("all {} generated artifacts are current", artifacts.len());
    }
    ExitCode::SUCCESS
}

fn build(m: &Manifest) -> Vec<Artifact> {
    vec![
        Artifact {
            // Run through rustfmt so `cargo fmt --all` is a no-op here. Without this, formatting the
            // workspace rewrites a generated file and `xtask check` fails on a change nobody made
            // — which is a real failure the first time and pure noise every time after.
            path: "crates/cv-api/src/lib.rs",
            body: rustfmt(emit_rust::emit(m)),
        },
        Artifact {
            path: "crates/cv-bindings/index.d.ts",
            body: emit_ts::emit(m),
        },
        Artifact {
            path: "editor/palette.json",
            body: emit_palette::emit(m),
        },
        Artifact {
            path: "docs/authoring/api-reference.md",
            body: emit_docs::emit(m),
        },
    ]
}

/// Format Rust source the way `cargo fmt` would, reading the workspace `rustfmt.toml`.
///
/// If rustfmt is missing or rejects the input the unformatted text is returned rather than failing:
/// a generator that cannot run without a formatter is a generator that breaks on a machine where
/// the component was not installed, and the `check` pass will report the difference anyway.
fn rustfmt(src: String) -> String {
    let mut child = match Command::new("rustfmt")
        .args(["--edition", "2021", "--emit", "stdout", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return src,
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(src.as_bytes()).is_err() {
            return src;
        }
    }
    match child.wait_with_output() {
        Ok(out) if out.status.success() => String::from_utf8(out.stdout).unwrap_or(src),
        _ => src,
    }
}

/// The workspace root, found by walking up from this crate.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/xtask is two levels below the root")
        .to_path_buf()
}

/// The header every generated file carries.
///
/// Worth being blunt: the single most likely way this system fails is somebody fixing a typo in a
/// generated file, and the fix surviving until the next regeneration silently reverts it.
pub(crate) fn banner(comment: &str) -> String {
    [
        format!("{comment} GENERATED — do not edit."),
        comment.to_string(),
        format!("{comment} Source: manifest/tier1.toml"),
        format!("{comment} Regenerate: cargo xtask generate"),
        comment.to_string(),
        format!("{comment} Every edit here is lost on the next run. Change the manifest instead — it is the"),
        format!("{comment} only file in this system that is authored by hand."),
    ]
    .join("\n")
}
