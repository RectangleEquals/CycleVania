//! `editor/palette.json` — the node palette the graph editor loads.
//!
//! This file is the structural argument the whole visual-authoring pivot rests on: **there is no
//! text field to type a wrong API name into**, because every node a developer can place is emitted
//! here from the manifest. An invented member is not a typo that compiles to a runtime error — it is
//! a node that does not exist in the list.
//!
//! # Node shapes
//!
//! | Manifest | Node | Exec pins |
//! |---|---|---|
//! | `field` | **get** | none — it is a pure read |
//! | `field` with `mutable` | get **and set** | the set node has them |
//! | `method` | **call** | yes |
//!
//! ⚠ That table is why `field` versus `method` is semantic rather than cosmetic. A member tagged
//! wrongly ships a wrong-shaped node, and the shape is what tells a developer whether the thing
//! costs anything — *the signal a method sends is cost*.
//!
//! Only `stable` members are emitted. A `proposed` member exists in the manifest and the reference
//! but must not be reachable from the palette, or content starts depending on it.

use cv_manifest::model::{Class, Kind, Status};
use cv_manifest::Manifest;
use std::fmt::Write;

pub fn emit(m: &Manifest) -> String {
    let mut s = String::new();
    // JSON has no comments, so the banner is a field. It is first so it survives a truncated read.
    let _ = writeln!(s, "{{");
    let _ = writeln!(
        s,
        "  \"$generated\": \"do not edit — regenerate with `cargo xtask generate`; source is manifest/tier1.toml\","
    );
    let _ = writeln!(s, "  \"version\": {},", m.version);
    let _ = writeln!(s, "  \"nodes\": [");

    let mut rows: Vec<String> = Vec::new();
    for c in &m.classes {
        if c.status != Status::Stable {
            continue;
        }
        if c.kind() == Kind::Enum {
            rows.push(enum_node(c));
            continue;
        }
        for f in c
            .fields
            .iter()
            .filter(|f| f.api && f.status == Status::Stable)
        {
            rows.push(field_node(c, &f.name, &f.ty, &f.doc, "get", false));
            if f.mutable {
                rows.push(field_node(c, &f.name, &f.ty, &f.doc, "set", true));
            }
        }
        for me in c
            .methods
            .iter()
            .filter(|m| m.api && m.status == Status::Stable)
        {
            rows.push(method_node(c, me));
        }
    }

    for (i, r) in rows.iter().enumerate() {
        let comma = if i + 1 == rows.len() { "" } else { "," };
        let _ = writeln!(s, "{r}{comma}");
    }
    let _ = writeln!(s, "  ]");
    let _ = writeln!(s, "}}");
    s
}

fn field_node(c: &Class, name: &str, ty: &str, doc: &str, verb: &str, exec: bool) -> String {
    let mut pins = vec![pin("self", &format!("Ref<{}>", c.short_name()), "in")];
    if verb == "get" {
        pins.push(pin("value", ty, "out"));
    } else {
        pins.push(pin("value", ty, "in"));
    }
    if exec {
        pins.insert(0, pin("in", "exec", "in"));
        pins.push(pin("out", "exec", "out"));
    }
    node(
        &format!("{}.{name}#{verb}", c.path),
        &format!("{verb} {name}"),
        c.short_name(),
        if exec { "call" } else { "pure" },
        doc,
        pins,
    )
}

fn method_node(c: &Class, me: &cv_manifest::Method) -> String {
    let mut pins = vec![
        pin("in", "exec", "in"),
        pin("self", &format!("Ref<{}>", c.short_name()), "in"),
    ];
    for p in &me.params {
        pins.push(pin(&p.name, &p.ty, "in"));
    }
    pins.push(pin("out", "exec", "out"));
    if me.returns != "void" {
        pins.push(pin("return", &me.returns, "out"));
    }
    node(
        &format!("{}.{}", c.path, me.name),
        &me.name,
        c.short_name(),
        "call",
        &me.doc,
        pins,
    )
}

fn enum_node(c: &Class) -> String {
    let values: Vec<String> = c.values.iter().map(|v| format!("{:?}", v.name)).collect();
    let mut s = String::new();
    let _ = writeln!(s, "    {{");
    let _ = writeln!(s, "      \"op\": {:?},", c.path);
    let _ = writeln!(s, "      \"label\": {:?},", c.short_name());
    let _ = writeln!(s, "      \"category\": \"Enum\",");
    let _ = writeln!(s, "      \"shape\": \"literal\",");
    let _ = writeln!(s, "      \"doc\": {:?},", c.doc);
    let _ = writeln!(s, "      \"values\": [{}],", values.join(", "));
    let _ = writeln!(
        s,
        "      \"pins\": [{{ \"name\": \"value\", \"type\": {:?}, \"dir\": \"out\" }}]",
        c.short_name()
    );
    let _ = write!(s, "    }}");
    s
}

fn pin(name: &str, ty: &str, dir: &str) -> String {
    format!("{{ \"name\": {name:?}, \"type\": {ty:?}, \"dir\": {dir:?} }}")
}

fn node(
    op: &str,
    label: &str,
    category: &str,
    shape: &str,
    doc: &str,
    pins: Vec<String>,
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "    {{");
    let _ = writeln!(s, "      \"op\": {op:?},");
    let _ = writeln!(s, "      \"label\": {label:?},");
    let _ = writeln!(s, "      \"category\": {category:?},");
    let _ = writeln!(s, "      \"shape\": {shape:?},");
    let _ = writeln!(s, "      \"doc\": {doc:?},");
    let _ = writeln!(s, "      \"pins\": [");
    for (i, p) in pins.iter().enumerate() {
        let comma = if i + 1 == pins.len() { "" } else { "," };
        let _ = writeln!(s, "        {p}{comma}");
    }
    let _ = writeln!(s, "      ]");
    let _ = write!(s, "    }}");
    s
}
