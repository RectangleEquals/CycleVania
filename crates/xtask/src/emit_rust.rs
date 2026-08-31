//! `crates/cv-api/src/lib.rs` — the manifest as Rust data.
//!
//! # Why a descriptor table rather than traits with bodies
//!
//! The obvious reading of "generate Rust from the manifest" is a trait per class. It is the wrong
//! artifact here, for two reasons:
//!
//! * **Content does not implement Rust traits.** Content is graphs, compiled to bytecode and
//!   dispatched by the VM (M13). A Rust trait would have exactly one implementor — the core — which
//!   is not what a trait is for.
//! * **The defaults are prose.** `default = "union of component collision"` describes behaviour the
//!   core already implements; it is not a body that can be generated.
//!
//! What the core, the VM, the inspector and the palette generator all actually need is the *same*
//! thing: a table they can look a member up in. So that is what this emits — and every consumer
//! reads one table rather than four hand-kept copies drifting apart.

use cv_manifest::model::{Class, Kind, Status};
use cv_manifest::Manifest;
use std::fmt::Write;

pub fn emit(m: &Manifest) -> String {
    let mut s = String::new();
    let b = crate::banner("//!");
    let _ = writeln!(
        s,
        "{b}\n//!\n//! # The tier-1 surface, as data\n//!\n//! {} declarations and {} members. Every other surface in the toolchain — the VM's dispatch\n//! table, the inspector, the node palette, the reference — reads this rather than restating it.\n",
        m.classes.len(),
        m.member_count()
    );

    let _ = write!(s, "{}", MODEL);

    let _ = writeln!(s, "/// Every tier-1 declaration, in manifest order.");
    let _ = writeln!(s, "///");
    let _ = writeln!(
        s,
        "/// Order is manifest order and not sorted: the manifest is the authored artifact, so a"
    );
    let _ = writeln!(
        s,
        "/// reordering there should be visible here rather than silently normalised away."
    );
    let _ = writeln!(s, "pub const CLASSES: &[ClassDesc] = &[");
    for c in &m.classes {
        emit_class(&mut s, c);
    }
    let _ = writeln!(s, "];\n");

    let _ = writeln!(
        s,
        "/// Declaration counts, asserted by the manifest's own tests."
    );
    let _ = writeln!(
        s,
        "pub const OBJECT_COUNT: usize = {};",
        m.count_of(Kind::Object)
    );
    let _ = writeln!(
        s,
        "pub const STRUCT_COUNT: usize = {};",
        m.count_of(Kind::Struct)
    );
    let _ = writeln!(
        s,
        "pub const ENUM_COUNT: usize = {};",
        m.count_of(Kind::Enum)
    );
    let _ = writeln!(s, "pub const MEMBER_COUNT: usize = {};", m.member_count());
    s
}

fn emit_class(s: &mut String, c: &Class) {
    let _ = writeln!(s, "    ClassDesc {{");
    let _ = writeln!(s, "        path: {:?},", c.path);
    let _ = writeln!(s, "        extends: {},", opt(c.extends.as_deref()));
    let _ = writeln!(s, "        kind: DeclKind::{},", kind_variant(c.kind()));
    let _ = writeln!(s, "        sealed: {},", c.sealed);
    let _ = writeln!(s, "        is_abstract: {},", c.is_abstract);
    let _ = writeln!(s, "        status: Status::{},", status_variant(c.status));
    let _ = writeln!(s, "        doc: {:?},", c.doc);

    let _ = writeln!(s, "        fields: &[");
    for f in &c.fields {
        let _ = writeln!(s, "            FieldDesc {{");
        let _ = writeln!(s, "                name: {:?},", f.name);
        let _ = writeln!(s, "                ty: {:?},", f.ty);
        let _ = writeln!(s, "                api: {},", f.api);
        let _ = writeln!(s, "                is_final: {},", f.is_final);
        let _ = writeln!(s, "                exposed: {},", f.exposed);
        let _ = writeln!(s, "                mutable: {},", f.mutable);
        let _ = writeln!(
            s,
            "                status: Status::{},",
            status_variant(f.status)
        );
        let _ = writeln!(s, "                doc: {:?},", f.doc);
        let _ = writeln!(s, "                default: {},", opt(f.default.as_deref()));
        let _ = writeln!(s, "            }},");
    }
    let _ = writeln!(s, "        ],");

    let _ = writeln!(s, "        methods: &[");
    for me in &c.methods {
        let _ = writeln!(s, "            MethodDesc {{");
        let _ = writeln!(s, "                name: {:?},", me.name);
        let _ = writeln!(s, "                params: &[");
        for p in &me.params {
            let _ = writeln!(
                s,
                "                    ParamDesc {{ name: {:?}, ty: {:?} }},",
                p.name, p.ty
            );
        }
        let _ = writeln!(s, "                ],");
        let _ = writeln!(s, "                returns: {:?},", me.returns);
        let _ = writeln!(s, "                api: {},", me.api);
        let _ = writeln!(s, "                is_final: {},", me.is_final);
        let _ = writeln!(s, "                is_abstract: {},", me.is_abstract);
        let _ = writeln!(s, "                hook: {},", me.hook);
        let _ = writeln!(
            s,
            "                status: Status::{},",
            status_variant(me.status)
        );
        let _ = writeln!(s, "                doc: {:?},", me.doc);
        let _ = writeln!(
            s,
            "                default: {},",
            opt(me.default.as_deref())
        );
        let _ = writeln!(s, "            }},");
    }
    let _ = writeln!(s, "        ],");

    let _ = writeln!(s, "        values: &[");
    for v in &c.values {
        let _ = writeln!(
            s,
            "            ValueDesc {{ name: {:?}, doc: {:?} }},",
            v.name, v.doc
        );
    }
    let _ = writeln!(s, "        ],");
    let _ = writeln!(s, "    }},");
}

fn opt(v: Option<&str>) -> String {
    match v {
        Some(x) => format!("Some({x:?})"),
        None => "None".into(),
    }
}

fn kind_variant(k: Kind) -> &'static str {
    match k {
        Kind::Object => "Object",
        Kind::Struct => "Struct",
        Kind::Enum => "Enum",
    }
}

fn status_variant(s: Status) -> &'static str {
    match s {
        Status::Proposed => "Proposed",
        Status::Stable => "Stable",
        Status::Deprecated => "Deprecated",
    }
}

/// The hand-shaped part of the generated file: the types the table is made of.
///
/// It is emitted rather than living in a separate hand-written module so that `cv-api` has exactly
/// one file and no question about which half is generated.
const MODEL: &str = r#"
/// Which family a declaration belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclKind {
    /// Has identity, may be subclassed.
    Object,
    /// Copied, no identity, not subclassable.
    Struct,
    /// A closed value set. Becomes a dropdown in the palette.
    Enum,
}

/// Release state. Generated palettes ship only `Stable`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Proposed,
    Stable,
    Deprecated,
}

/// One argument of a method.
///
/// There is deliberately no `default` here: the binding contract forbids defaults in an FFI
/// signature, so the schema gives them nowhere to live.
#[derive(Clone, Copy, Debug)]
pub struct ParamDesc {
    pub name: &'static str,
    pub ty: &'static str,
}

/// A plain read of something already known. Becomes a pure get node in the palette.
#[derive(Clone, Copy, Debug)]
pub struct FieldDesc {
    pub name: &'static str,
    pub ty: &'static str,
    pub api: bool,
    pub is_final: bool,
    pub exposed: bool,
    pub mutable: bool,
    pub status: Status,
    pub doc: &'static str,
    /// The *behaviour* of the default, in prose. The editor shows this rather than the word
    /// "inherited", because a developer needs to know what happens, not that something happens.
    pub default: Option<&'static str>,
}

/// Something that takes an argument, computes, or mutates. Becomes a call node with exec pins.
#[derive(Clone, Copy, Debug)]
pub struct MethodDesc {
    pub name: &'static str,
    pub params: &'static [ParamDesc],
    pub returns: &'static str,
    pub api: bool,
    pub is_final: bool,
    pub is_abstract: bool,
    /// A question the core asks. Drives a schematic's OVERRIDES list.
    pub hook: bool,
    pub status: Status,
    pub doc: &'static str,
    pub default: Option<&'static str>,
}

/// One member of an enum.
#[derive(Clone, Copy, Debug)]
pub struct ValueDesc {
    pub name: &'static str,
    pub doc: &'static str,
}

/// One tier-1 declaration.
#[derive(Clone, Copy, Debug)]
pub struct ClassDesc {
    pub path: &'static str,
    pub extends: Option<&'static str>,
    pub kind: DeclKind,
    pub sealed: bool,
    pub is_abstract: bool,
    pub status: Status,
    pub doc: &'static str,
    pub fields: &'static [FieldDesc],
    pub methods: &'static [MethodDesc],
    pub values: &'static [ValueDesc],
}

impl ClassDesc {
    /// The last path segment — `Actor` for `/Core/Actor`.
    pub fn short_name(&self) -> &'static str {
        match self.path.rfind('/') {
            Some(i) => &self.path[i + 1..],
            None => self.path,
        }
    }

    /// Members the core asks about, which is what a schematic's OVERRIDES list is built from.
    pub fn hooks(&self) -> impl Iterator<Item = &MethodDesc> {
        self.methods.iter().filter(|m| m.hook)
    }
}

/// Look one declaration up by mount-pointed path.
pub fn find(path: &str) -> Option<&'static ClassDesc> {
    CLASSES.iter().find(|c| c.path == path)
}

/// Walk a declaration's ancestry, nearest first.
pub fn ancestors(mut c: &'static ClassDesc) -> Vec<&'static ClassDesc> {
    let mut out = Vec::new();
    while let Some(parent) = c.extends.and_then(find) {
        out.push(parent);
        c = parent;
    }
    out
}
"#;
