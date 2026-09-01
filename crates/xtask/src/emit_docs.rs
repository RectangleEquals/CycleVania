//! `docs/authoring/api-reference.md` — the developer-facing API reference.
//!
//! # Why this replaces the design's hand-written reference
//!
//! `.notes/Design/v0.2b/06-api/reference.md` describes itself as *"hand-maintained seed data for
//! the manifest. Temporary."* M00 consumed it; from here the reference is an **output**, and the
//! design document is history.
//!
//! The two are not byte-identical and were never going to be. The design version carries narrative
//! the manifest cannot hold — worked examples, the four-answers glass table, the argument for why
//! dials inherit outward-in. Most of *that* is now design prose living in the design; what a
//! developer needs while authoring is the signature, the default, and one sentence of why. That is
//! what this emits, and it goes to `docs/authoring/` because the audience is a content author.

use cv_manifest::model::{Class, Kind, Status};
use cv_manifest::Manifest;
use std::fmt::Write;

pub fn emit(m: &Manifest) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "<!--\n{}\n-->\n", crate::banner(""));
    let _ = writeln!(s, "# API reference\n");
    let _ = writeln!(
        s,
        "The tier-1 surface: **{} declarations** and **{} members**, generated from the manifest.\n",
        m.classes.len(),
        m.member_count()
    );
    let _ = writeln!(
        s,
        "Notation: a **field** is a plain read and appears in a graph as a pure *get* node. A \
         **method** takes an argument, computes, or mutates, and appears as a *call* node with \
         execution pins — so the shape of a node tells you whether it costs anything. A **hook** is a \
         question the core asks; hooks are what a schematic's `OVERRIDES` list is built from.\n"
    );
    let _ = writeln!(s, "Paths are mount-pointed: `/Core/…` is tier-1.\n");

    for (heading, kind) in [
        ("Objects", Kind::Object),
        ("Structs", Kind::Struct),
        ("Variants", Kind::Variant),
        ("Enums", Kind::Enum),
    ] {
        let _ = writeln!(s, "---\n\n## {heading}\n");
        for c in m.classes.iter().filter(|c| c.kind() == kind) {
            emit_class(&mut s, c);
        }
    }
    s
}

fn emit_class(s: &mut String, c: &Class) {
    let _ = write!(s, "### `{}`", c.short_name());
    if let Some(p) = &c.extends {
        let _ = write!(s, " — extends `{}`", p.rsplit('/').next().unwrap_or(p));
    }
    let _ = writeln!(s, "\n");

    let mut badges = Vec::new();
    if c.sealed {
        badges.push("**sealed** — content may not subclass this");
    }
    if c.is_abstract {
        badges.push("**abstract** — a subclass must answer");
    }
    if c.status == Status::Proposed {
        badges.push("▶ **PROPOSED** — may change or be removed");
    }
    if c.status == Status::Deprecated {
        badges.push("⚠ **DEPRECATED**");
    }
    if !badges.is_empty() {
        let _ = writeln!(s, "{}\n", badges.join(" · "));
    }

    let _ = writeln!(s, "{}\n", c.doc);
    let _ = writeln!(s, "`{}`\n", c.path);

    if !c.values.is_empty() {
        let _ = writeln!(s, "| Value | |");
        let _ = writeln!(s, "|---|---|");
        for v in &c.values {
            let _ = writeln!(s, "| `{}` | {} |", v.name, v.doc);
        }
        let _ = writeln!(s);
    }

    if !c.fields.is_empty() {
        let _ = writeln!(s, "| Field | Type | | |");
        let _ = writeln!(s, "|---|---|---|---|");
        for f in &c.fields {
            let mut flags = Vec::new();
            if f.mutable {
                flags.push("mutable");
            }
            if f.exposed {
                flags.push("exposed");
            }
            if f.is_final {
                flags.push("final");
            }
            if f.status == Status::Proposed {
                flags.push("▶ proposed");
            }
            let mut note = f.doc.clone();
            if let Some(d) = &f.default {
                let _ = write!(note, " *Default: {d}.*");
            }
            let _ = writeln!(
                s,
                "| `{}` | `{}` | {} | {} |",
                f.name,
                f.ty,
                flags.join(" · "),
                note
            );
        }
        let _ = writeln!(s);
    }

    if !c.methods.is_empty() {
        let _ = writeln!(s, "| Method | Returns | | |");
        let _ = writeln!(s, "|---|---|---|---|");
        for me in &c.methods {
            let args: Vec<String> = me
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, p.ty))
                .collect();
            let mut flags = Vec::new();
            if me.hook {
                flags.push("**hook**");
            }
            if me.is_final {
                flags.push("final");
            }
            if me.is_abstract {
                flags.push("abstract");
            }
            if me.status == Status::Proposed {
                flags.push("▶ proposed");
            }
            let mut note = me.doc.clone();
            if let Some(d) = &me.default {
                let _ = write!(note, " *Default: {d}.*");
            }
            let _ = writeln!(
                s,
                "| `{}({})` | `{}` | {} | {} |",
                me.name,
                args.join(", "),
                me.returns,
                flags.join(" · "),
                note
            );
        }
        let _ = writeln!(s);
    }
}
