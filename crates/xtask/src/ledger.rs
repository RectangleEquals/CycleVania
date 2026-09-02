//! **The design ledger** — turning *"did we miss anything?"* from a reading question into a build one.
//!
//! # The problem this exists to end
//!
//! Every previous check was an **ad-hoc projection** of the design: one pass matched `/Core/X`, another
//! matched the type tree, another matched output tables. Each new projection found what the previous
//! ones did not cover, so each audit found more — not because the auditing improved, but because an
//! unbounded search has no termination condition. `CV_*` metadata was specified in two documents,
//! enumerated in one, and picked up by no milestone; nothing was *wrong* in any file, so no amount of
//! reading either file would have surfaced it.
//!
//! ⚠ **Reading cannot establish absence.** It establishes only that nothing caught the reader's eye.
//!
//! # What replaces it
//!
//! One **closed** extraction. Every enumerable surface in the design — declared classes, their members,
//! the type tree, and every table row that names something — becomes a ledger entry. Each entry carries
//! a **disposition**:
//!
//! | Disposition | Means |
//! |---|---|
//! | `present` | a manifest declaration **or** a Rust name exists for it |
//! | `owed` | a milestone in the plan names it |
//! | `declared` | a tier-1 manifest declaration; `coverage.rs` owns whether it is implemented |
//! | `refused` | the design's own deliberately-absent table names it |
//! | **`none`** | ⚠ **nobody has looked at this** |
//!
//! ⚠ **`present` deliberately does not mean *implemented*, and it used to be called `built`, which
//! read as though it did.** A tier-1 class can be declared in the manifest and have no Rust behind it
//! — `ScopeHandle` is — so the old name had this ledger reporting *built* for a surface `coverage.rs`
//! simultaneously reported *owed*. Two ratchets contradicting each other is worse than one, because a
//! reader believes whichever they opened. **The division of labour:** this ledger answers *has anyone
//! taken responsibility for this surface* over the whole design; `coverage.rs` answers *is this tier-1
//! declaration implemented*. Neither can answer the other's question, and the names now say so.
//!
//! The committed ledger records the count of each. A test fails when the `none` count **rises**. So the
//! existing backlog is visible and worked down deliberately, while a new design surface that nobody has
//! dispositioned breaks the build the moment it is written.
//!
//! ⚠ **That is the termination condition.** *"Have we missed something?"* stops being a matter of how
//! hard someone looked and becomes a number that is either zero or not.
//!
//! # The ledger is as private as the notes it projects
//!
//! ⚠ **Every row names a design file and a line number.** That makes the projection exactly as private
//! as its source, so it lives *inside* the already-private notes — `.notes/Implementation/v0.2b/` —
//! rather than in the public `docs/` tree behind an ignore rule.
//!
//! An ignore rule would have been one line standing between a private file and a public repository: a
//! `git add -f`, a `.gitignore` refactor, or a tool that writes its own rules re-exposes it — and the
//! rule's very presence advertises that a file by that name exists. A file under `.notes/**` is
//! covered by a directory-wide rule that predates it and is not going to move.
//!
//! ⚠ **So this is a local artifact, not a build output.** Whoever holds the design regenerates it with
//! `cargo xtask ledger`; a clone without the notes has no ledger, and every check that reads one treats
//! its absence as normal rather than as failure.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

/// Where the design lives, relative to the repo root. Gitignored, so absence is normal.
pub const DESIGN_DIR: &str = ".notes/Design/v0.2b";
/// Where the plan lives. Also gitignored.
pub const PLAN_DIR: &str = ".notes/Implementation/v0.2b";
/// Where the ledger is written — **inside the private notes**, for the reason in the module docs.
pub const LEDGER_PATH: &str = ".notes/Implementation/v0.2b/design-ledger.md";

/// What kind of design surface an entry came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Surface {
    /// An `api class /Core/X` declaration.
    Class,
    /// An `api func` or `api var` on one.
    Member,
    /// A name in the object-model type tree.
    Tree,
    /// The first cell of a table row that names something.
    ///
    /// ⚠ **The category `CV_*` fell into**, and the reason the extraction includes it at all: a design
    /// table row is an *enumeration*, and an enumeration is exactly the thing a reader skims.
    Row,
    /// A tier-1 declaration in `manifest/tier1.toml`.
    ///
    /// ⚠ **The manifest is a design artefact, and leaving it out left this extraction open.** Reading
    /// only the prose missed 37 declarations `coverage.rs` tracks — `MinDistanceFrom`, `MountedOn`,
    /// `SpherePin`, `PlacedAfter`, `SkipPolicy`, `Detail`, `Fidelity` and more. Every one *is* in the
    /// design, written in a form this extractor cannot see: inside a code block, on an `api class`
    /// **continuation** line, as a member's *type* rather than its name, or in a heading. Widening the
    /// prose scan to catch those would match ordinary English and produce a number that looks complete
    /// and means nothing — the mistake `names_it` already documents. **So the manifest is read as
    /// itself.**
    Declaration,
}

impl Surface {
    fn as_str(self) -> &'static str {
        match self {
            Surface::Class => "class",
            Surface::Member => "member",
            Surface::Tree => "tree",
            Surface::Row => "row",
            Surface::Declaration => "declaration",
        }
    }
}

/// What has happened to a surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Disposition {
    /// A manifest declaration or a Rust name exists for it.
    ///
    /// ⚠ **Not a claim that it is implemented** — see the module header. `coverage.rs` owns that
    /// question for tier-1 declarations.
    Present,
    /// A milestone names it.
    Owed,
    /// The design's own deliberately-absent table names it.
    Refused,
    /// A tier-1 manifest declaration, whose *implementation* status is `coverage.rs`'s question.
    ///
    /// ⚠ **Not folded into `present`, which would have been circular** — a manifest-sourced entry is
    /// in the manifest by construction, so calling it *present* on that evidence proves nothing. This
    /// value says what is actually known: the surface is **declared**, and the other ratchet answers
    /// whether anything implements it.
    Declared,
    /// ⚠ **Nobody has looked at this.**
    None,
}

impl Disposition {
    fn as_str(self) -> &'static str {
        match self {
            Disposition::Present => "present",
            Disposition::Declared => "declared",
            Disposition::Owed => "owed",
            Disposition::Refused => "refused",
            Disposition::None => "none",
        }
    }
}

/// One design surface, and what happened to it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    pub name: String,
    pub surface: Surface,
    pub disposition: Disposition,
    /// Where in the design it came from — `file.md:line`.
    pub origin: String,
}

/// Read every design file, or `None` if the design is not present.
fn read_dir_md(root: &Path, rel: &str) -> Option<Vec<(String, String)>> {
    let dir = root.join(rel);
    if !dir.is_dir() {
        return None;
    }
    let mut out = Vec::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                if let Ok(s) = std::fs::read_to_string(&p) {
                    let name = p
                        .strip_prefix(root)
                        .unwrap_or(&p)
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push((name, s));
                }
            }
        }
    }
    out.sort();
    Some(out)
}

/// Pull every backticked identifier out of a line.
fn identifiers(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(a) = rest.find('`') {
        rest = &rest[a + 1..];
        let Some(b) = rest.find('`') else { break };
        let tok = &rest[..b];
        rest = &rest[b + 1..];
        // A name, not a sentence: no spaces, starts with a letter or a slash.
        let head = tok.trim_start_matches('/');
        if !tok.contains(' ')
            && tok.len() >= 3
            && head.starts_with(|c: char| c.is_ascii_alphabetic())
            && tok
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "/_<>.:".contains(c))
        {
            out.push(tok.to_string());
        }
    }
    out
}

/// The bare type name a surface refers to — `/Core/Item` and `Item` are one thing.
///
/// ⚠ Trailing punctuation is stripped, because a declaration line reads `api class /Core/Route:` and a
/// `Route:` entry would be a **phantom finding** — noise that makes the real count untrustworthy.
fn short(name: &str) -> &str {
    let n = name
        .rsplit('/')
        .next()
        .unwrap_or(name)
        .split('<')
        .next()
        .unwrap_or(name);
    let n = n.trim_end_matches([':', '.', ',', ';']);
    // ⚠ A dotted reference names a *member* — `Item.classification` is the `classification` hook, and
    // treating the whole string as one surface would report a member that exists as undispositioned.
    n.rsplit('.').next().unwrap_or(n)
}

/// Is this an annotation rather than a name?
///
/// ⚠ The design's type tree carries prose in the same columns as names — `NON-BEHAVIORAL core API`,
/// `(SEALED)`, `ONE ROW of a CurveTableResource`. Counting those as undispositioned surfaces would bury
/// the real ones, and a signal nobody trusts is worse than no signal.
fn is_prose(tok: &str) -> bool {
    tok.chars().all(|c| c.is_ascii_uppercase() || c == '_')
}

/// Extract every enumerable design surface.
///
/// ⚠ **Closed by construction, not by judgement.** Everything matched here is matched by *shape* — a
/// declaration line, a tree entry, a table row — so the set does not depend on what a reader thought
/// was interesting.
pub fn extract(root: &Path) -> Option<Vec<Entry>> {
    let files = read_dir_md(root, DESIGN_DIR)?;
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen: BTreeSet<(String, Surface)> = BTreeSet::new();

    let mut push = |name: &str, surface: Surface, origin: String, entries: &mut Vec<Entry>| {
        let key = (name.to_string(), surface);
        if seen.insert(key) {
            entries.push(Entry {
                name: name.to_string(),
                surface,
                disposition: Disposition::None,
                origin,
            });
        }
    };

    for (file, body) in &files {
        // The changelog records history, not surface.
        if file.ends_with("CHANGELOG.md") {
            continue;
        }
        let mut in_tree = false;
        for (i, line) in body.lines().enumerate() {
            let origin = format!("{file}:{}", i + 1);

            // --- declared classes ---
            if let Some(rest) = line.trim_start().strip_prefix("api ") {
                let rest = rest
                    .trim_start_matches("final ")
                    .trim_start_matches("abstract ");
                if let Some(p) = rest.strip_prefix("class ") {
                    if let Some(path) = p.split_whitespace().next() {
                        push(short(path), Surface::Class, origin.clone(), &mut entries);
                    }
                } else if let Some(f) = rest.strip_prefix("func ") {
                    if let Some(n) = f.split('(').next() {
                        push(n.trim(), Surface::Member, origin.clone(), &mut entries);
                    }
                } else if let Some(v) = rest
                    .strip_prefix("var ")
                    .or_else(|| rest.strip_prefix("mutable var "))
                {
                    if let Some(n) = v.split(&[':', ' '][..]).next() {
                        push(n.trim(), Surface::Member, origin.clone(), &mut entries);
                    }
                }
            }

            // --- the object-model type tree ---
            if line.starts_with("Object ")
                || line.starts_with("Struct")
                || line.starts_with("Variant")
            {
                in_tree = true;
            } else if in_tree && line.trim().is_empty() {
                in_tree = false;
            }
            if in_tree && line.starts_with(['├', '└', '│', ' ']) {
                for tok in line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
                    if tok.len() > 2
                        && tok.starts_with(|c: char| c.is_ascii_uppercase())
                        && !is_prose(tok)
                    {
                        push(tok, Surface::Tree, origin.clone(), &mut entries);
                    }
                }
            }

            // --- table rows ---
            //
            // ⚠ The category the miss belonged to. A row's first cell names a thing the row is *about*,
            // and a table is where a design enumerates rather than argues.
            if line.starts_with('|') && !line.contains("---") {
                if let Some(first) = line.trim_start_matches('|').split('|').next() {
                    for id in identifiers(first) {
                        push(short(&id), Surface::Row, origin.clone(), &mut entries);
                    }
                }
            }
        }
    }
    // ⚠ **The manifest, read as itself** — and only for names the prose scan did not already reach.
    // Pushing every declaration unconditionally would double-count the ~100 the design also names, and
    // a total that counts one surface twice is the same species of meaningless number as one that
    // counts prose.
    let seen_names: BTreeSet<String> = entries.iter().map(|e| e.name.clone()).collect();
    if let Ok(man) = std::fs::read_to_string(root.join("manifest/tier1.toml")) {
        let mut member_ctx = false;
        for (i, line) in man.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("[[class.") {
                member_ctx = true;
                continue;
            }
            if t.starts_with("[[class]]") {
                member_ctx = false;
                continue;
            }
            let name = if let Some(v) = t.strip_prefix("path = \"") {
                v.trim_end_matches('"')
                    .rsplit('/')
                    .next()
                    .map(str::to_string)
            } else if member_ctx {
                t.strip_prefix("name = \"")
                    .map(|v| v.trim_end_matches('"').to_string())
            } else {
                None
            };
            let Some(name) = name else { continue };
            if name.is_empty() || seen_names.contains(&name) {
                continue;
            }
            push(
                &name,
                Surface::Declaration,
                format!("manifest/tier1.toml:{}", i + 1),
                &mut entries,
            );
        }
    }

    entries.sort();
    Some(entries)
}

/// Assign a disposition to each entry from the manifest, the code and the plan.
pub fn dispose(root: &Path, entries: &mut [Entry]) {
    let manifest = std::fs::read_to_string(root.join("manifest/tier1.toml")).unwrap_or_default();
    let plan = read_dir_md(root, PLAN_DIR)
        .map(|v| v.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    let design = read_dir_md(root, DESIGN_DIR)
        .map(|v| v.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();

    // ⚠ **The design says "no" in more than one place**, and reading only one of them reported a
    // deliberate refusal as an undispositioned hole. `05` has "Deliberately absent"; `13-open-gaps`
    // §7 has "Deliberately out of scope" — same statement, different document, and a checker that
    // knew about one was quietly wrong about the other.
    let mut refused: BTreeSet<String> = BTreeSet::new();
    for marker in ["Deliberately absent", "Deliberately out of scope"] {
        for section in design.split(marker).skip(1) {
            for line in section.lines().take(24) {
                if !line.starts_with('|') {
                    continue;
                }
                let first = line.trim_start_matches('|').split('|').next().unwrap_or("");
                for id in identifiers(first) {
                    refused.insert(short(&id).to_string());
                }
            }
        }
    }

    let mut code = String::new();
    for dir in ["crates/cv-core/src", "crates/cv-determinism/src"] {
        if let Ok(rd) = std::fs::read_dir(root.join(dir)) {
            for e in rd.flatten() {
                if let Ok(s) = std::fs::read_to_string(e.path()) {
                    code.push_str(&s);
                }
            }
        }
    }

    for e in entries.iter_mut() {
        let n = e.name.as_str();
        // ⚠ **A manifest-sourced entry cannot earn `present` from the manifest.** That is the
        // circularity the `Declared` value exists to avoid: only real code counts here.
        if e.surface == Surface::Declaration {
            e.disposition = if refused.contains(n) {
                Disposition::Refused
            } else if code.contains(&format!("struct {n}"))
                || code.contains(&format!("enum {n}"))
                || code.contains(&format!("fn {n}"))
                || code.contains(&format!("pub {n}:"))
            {
                Disposition::Present
            } else {
                Disposition::Declared
            };
            continue;
        }
        if refused.contains(n) {
            e.disposition = Disposition::Refused;
        } else if manifest.contains(&format!("\"/Core/{n}\""))
            || manifest.contains(&format!("name = \"{n}\""))
            || code.contains(&format!("struct {n}"))
            || code.contains(&format!("enum {n}"))
            || code.contains(&format!("fn {n}"))
            // ⚠ A design surface may be a *field*, and the detector only knew about types and
            // functions — so `ResourceDef.regenerates`, which exists, reported as a hole.
            || code.contains(&format!("pub {n}:"))
        {
            e.disposition = Disposition::Present;
        } else if names_it(&plan, n) {
            e.disposition = Disposition::Owed;
        }
    }
}

/// Does the plan **name** this surface, rather than merely contain the word?
///
/// ⚠ **A substring match is not a disposition.** `api`, `hook`, `exec` and `crawler` all appear in the
/// plan as ordinary English, and matching on that reported them as *owed by a milestone* — a number
/// that looked complete and meant nothing. A plan names a surface the way this codebase always does:
/// in backticks, or bolded. Anything looser is prose agreeing with prose.
fn names_it(plan: &str, name: &str) -> bool {
    plan.contains(&format!("`{name}`"))
        || plan.contains(&format!("**{name}**"))
        || plan.contains(&format!("`{name}("))
        || plan.contains(&format!("`{name}."))
        || plan.contains(&format!(".{name}`"))
        || plan.contains(&format!("::{name}`"))
}

/// Render the committed ledger.
pub fn render(entries: &[Entry]) -> String {
    let mut s = String::new();
    // Not `crate::banner` — that one names the manifest as the source, and this file's source is
    // the design. A generated file advertising the wrong regeneration command gets hand-edited.
    let _ = writeln!(
        s,
        "<!-- GENERATED — do not edit.\n\
         <!--\n\
         <!-- Source: .notes/Design/v0.2b - PRIVATE, and so is this projection of it\n\
         <!-- Regenerate: cargo xtask ledger\n"
    );
    let _ = writeln!(s, "# Design ledger\n");
    let _ = writeln!(
        s,
        "Every enumerable surface in the v0.2b design, and what happened to it. Regenerate with\n\
         `cargo xtask ledger`; `cargo xtask check` fails if this file is stale.\n"
    );
    let _ = writeln!(
        s,
        "**`none` is the number that matters.** It counts surfaces nobody has answered for — the\n\
         category `CV_*` metadata sat in while it was specified, guarded and unbuilt. `coverage.rs`\n\
         fails when it rises above zero, so a design surface nobody dispositions breaks the build\n\
         the moment it is written.\n\n\
         **What `owed` does and does not prove.** It means a milestone *names* the surface — that\n\
         someone wrote it down, not that the plan for it is right. Conformance is the per-milestone\n\
         tests job; this file answers only whether anyone has taken responsibility.\n"
    );

    let mut counts = [0usize; 5];
    for e in entries {
        counts[e.disposition as usize] += 1;
    }
    let _ = writeln!(s, "| Disposition | Count |");
    let _ = writeln!(s, "|---|---|");
    for (d, label) in [
        (Disposition::Present, "present"),
        (Disposition::Owed, "owed"),
        (
            Disposition::Declared,
            "declared — `coverage.rs` owns whether it is implemented",
        ),
        (Disposition::Refused, "refused"),
        (Disposition::None, "**none — undispositioned**"),
    ] {
        let _ = writeln!(s, "| {label} | {} |", counts[d as usize]);
    }
    let _ = writeln!(s, "| **total** | {} |\n", entries.len());

    let _ = writeln!(s, "## Undispositioned\n");
    let undone: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.disposition == Disposition::None)
        .collect();
    if undone.is_empty() {
        let _ = writeln!(s, "_None._\n");
    } else {
        let _ = writeln!(s, "| Surface | Kind | First seen |");
        let _ = writeln!(s, "|---|---|---|");
        for e in &undone {
            let _ = writeln!(
                s,
                "| `{}` | {} | {} |",
                e.name,
                e.surface.as_str(),
                e.origin
            );
        }
        let _ = writeln!(s);
    }

    let _ = writeln!(s, "## Everything\n");
    let _ = writeln!(s, "| Surface | Kind | Disposition | First seen |");
    let _ = writeln!(s, "|---|---|---|---|");
    for e in entries {
        let _ = writeln!(
            s,
            "| `{}` | {} | {} | {} |",
            e.name,
            e.surface.as_str(),
            e.disposition.as_str(),
            e.origin
        );
    }
    s
}

/// Build the ledger, or `None` where the design is not checked out.
pub fn build(root: &Path) -> Option<String> {
    let mut entries = extract(root)?;
    dispose(root, &mut entries);
    Some(render(&entries))
}
