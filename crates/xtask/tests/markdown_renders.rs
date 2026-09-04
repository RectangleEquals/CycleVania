//! Every committed markdown file actually renders.
//!
//! ⚠ **The design ledger spent ten milestones as a blank page.** Its header wrote `<!--` on four
//! consecutive lines and closed none, so every renderer treated the whole 48 KB document as one
//! unterminated HTML comment and showed nothing. The content was correct the entire time.
//!
//! ⚠ **Nothing noticed, and every check that could have was green.** `xtask check` compares bytes, and
//! matched. The tests that read the ledger parsed counts out of it, and passed. **No check asked
//! whether a human could read the file** — which is the only thing a document is for.
//!
//! ▶ **So this is not a check on generated files, it is a check on committed ones.** A generated file
//! has no reader until someone opens it, but neither does a hand-written one; the failure mode belongs
//! to markdown, not to generation. `README.md` is the file most likely to be read and least likely to
//! be re-read by its author.
//!
//! # Counting, not parsing
//!
//! ⚠ **A markdown parser here would be a dependency and a second opinion.** The defect is unbalanced
//! delimiters, and balance is countable — a check whose own correctness needs checking is not one.

use std::path::PathBuf;
use std::process::Command;

fn committed_markdown() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("git")
        .args(["ls-files", "-z", "*.md"])
        .current_dir(&root)
        .output()
        .expect("git is required to know what is committed");
    let files: Vec<PathBuf> = String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|rel| root.join(rel))
        .collect();
    assert!(
        !files.is_empty(),
        "listed no markdown — a check that checks nothing passes every time"
    );
    files
}

/// `<!--` and `-->` come in pairs.
fn balanced(md: &str) -> Result<(), String> {
    let opens = md.matches("<!--").count();
    let closes = md.matches("-->").count();
    if opens == closes {
        return Ok(());
    }
    Err(format!(
        "{opens} `<!--` and {closes} `-->` — an unclosed comment makes a renderer swallow everything \
         after it, so the file stays valid to every check and invisible to every reader"
    ))
}

#[test]
fn no_committed_markdown_hides_itself_in_a_comment() {
    let mut broken = Vec::new();
    for path in committed_markdown() {
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Err(why) = balanced(&body) {
            broken.push(format!("{}: {why}", path.display()));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n  "));
}

#[test]
fn a_header_comment_closes_before_the_first_heading() {
    // ⚠ **Balance alone is not enough.** A file whose comment closes *after* its title is balanced and
    // still opens with nothing — the ledger's exact failure, one step less severe.
    let mut hidden = Vec::new();
    for path in committed_markdown() {
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (Some(open), Some(heading)) = (body.find("<!--"), body.find("\n# ")) else {
            continue;
        };
        if open > heading {
            continue; // the comment is below the title; nothing is being hidden
        }
        match body.find("-->") {
            Some(close) if close < heading => {}
            _ => hidden.push(format!(
                "{}: a comment opens at {open} and does not close before the title at {heading}",
                path.display()
            )),
        }
    }
    assert!(hidden.is_empty(), "{}", hidden.join("\n  "));
}

/// A guard nobody has seen fail is a guard nobody knows works.
#[test]
fn the_check_actually_catches_an_unclosed_comment() {
    assert!(balanced("<!-- GENERATED\n# Title\n").is_err());
    assert!(balanced("<!-- fine -->\n# Title\n").is_ok());
    // and the four-opener shape that actually happened
    assert!(balanced("<!-- a\n<!-- b\n<!-- c\n<!-- d\n# Title\n").is_err());
}
