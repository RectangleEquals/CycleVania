//! The constraints the generator refuses to build through.
//!
//! These are not style rules. Each one, violated, produces a specific downstream failure that is
//! expensive to find later:
//!
//! * a `u64` crossing the seam corrupts silently in TypeScript above 2^53
//! * an overloaded name has no Rust representation, so one of the two overloads vanishes
//! * a fourth inheritance level ships in every generated palette before anyone notices
//! * a `mutable` member that is not `exposed` is writable by nothing and readable by nobody
//! * a dangling `extends` or type reference generates code that does not compile

use crate::model::{Class, Kind, Manifest, Status};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    pub rule: &'static str,
    pub where_: String,
    pub detail: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.rule, self.where_, self.detail)
    }
}

/// Maximum inheritance depth from `/Core/Object`, counted in `extends` hops.
///
/// Three is not arbitrary: it is `Object → Actor → Item`, and the tree as designed never needs a
/// fourth. Nothing stops one being added later except this check, and by then a palette has shipped.
pub const MAX_DEPTH: usize = 3;

/// Integer widths that must never appear in a signature.
const BANNED_TYPES: [&str; 2] = ["i64", "u64"];

pub fn validate(m: &Manifest) -> Vec<Violation> {
    let mut v = Vec::new();
    unique_paths(m, &mut v);
    references_resolve(m, &mut v);
    inheritance_depth(m, &mut v);
    mutable_requires_exposed(m, &mut v);
    no_wide_integers(m, &mut v);
    no_overloads(m, &mut v);
    documented(m, &mut v);
    kind_consistency(m, &mut v);
    values_are_never_referenced(m, &mut v);
    lattice_is_not_bounded_at_the_root(m, &mut v);
    v.sort();
    v
}

/// The progression lattice may never be typed `Kind<Object>`.
///
/// # Why this is a rule and not a review note
///
/// `Object` is the root of everything, so a `Kind<Object>` pin bounds the editor's picker at *the whole
/// project*: every Actor, Component, Rule and authored schematic. A developer answering *"what does the
/// Hookshot grant?"* could pick a door class, nothing downstream would reject it, and the lock it was
/// meant to open would be **silently ungated**.
///
/// ⚠ That is a typed-string failure wearing a picker's clothes — the exact class of error the visual
/// pivot exists to prevent, reintroduced by a type that still compiles. The lattice trades in `Unlock`
/// rows, and this check keeps it that way: widening it back is a build failure, not a review catch.
fn lattice_is_not_bounded_at_the_root(m: &Manifest, out: &mut Vec<Violation>) {
    /// Members that carry lattice atoms, by the name the design gives them.
    const LATTICE: &[&str] = &["grants", "held", "granted_here", "referenced", "unlock"];

    let mut flag = |c: &Class, member: &str, ty: &str| {
        if LATTICE.contains(&member) && split_type(ty).iter().any(|p| p == "Object") {
            out.push(Violation {
                rule: "lattice-bound",
                where_: format!("{}::{member}", c.path),
                detail: format!(
                    "`{ty}` bounds the picker at the root of everything; the lattice trades in `Unlock`"
                ),
            });
        }
    };
    for c in &m.classes {
        for f in &c.fields {
            flag(c, &f.name, &f.ty);
        }
        for me in &c.methods {
            flag(c, &me.name, &me.returns);
            // ⚠ Params are checked on the PARAM name, not the method's: `accessible(from, to, held)`
            // carries a lattice set in `held`, and matching on `accessible` would miss it.
            for p in &me.params {
                flag(c, &p.name, &p.ty);
            }
        }
    }
}

fn unique_paths(m: &Manifest, out: &mut Vec<Violation>) {
    let mut seen = BTreeSet::new();
    for c in &m.classes {
        if c.path.is_empty() {
            out.push(Violation {
                rule: "path",
                where_: "(anonymous)".into(),
                detail: "a class declares no `path`".into(),
            });
        } else if !seen.insert(c.path.clone()) {
            out.push(Violation {
                rule: "path",
                where_: c.path.clone(),
                detail: "declared more than once".into(),
            });
        }
    }
}

/// Every `extends` target and every type named in a signature must resolve.
///
/// Types are matched on their *short* name, because a signature writes `Ref<Actor>` rather than the
/// mount-pointed path. Primitives and the container shells are known separately.
fn references_resolve(m: &Manifest, out: &mut Vec<Violation>) {
    let known: BTreeSet<&str> = m
        .classes
        .iter()
        .map(|c| c.short_name())
        .chain([
            // primitives and shells the schema uses without declaring
            "bool",
            "int",
            "float",
            "String",
            "void",
            "exec",
            "ObjectId",
            "Bytes",
            "Array",
            "Map",
            "Ref",
            "Kind",
            "Resource",
            "Query",
            "SpineSlot",
            "CollisionData",
            "PathStep",
            "T",
        ])
        .collect();

    for c in &m.classes {
        if let Some(parent) = &c.extends {
            if m.get(parent).is_none() {
                out.push(Violation {
                    rule: "extends",
                    where_: c.path.clone(),
                    detail: format!("`{parent}` is not declared"),
                });
            }
        }
        let mut check = |ty: &str, member: &str| {
            for part in split_type(ty) {
                if !known.contains(part.as_str()) {
                    out.push(Violation {
                        rule: "type",
                        where_: format!("{}::{member}", c.path),
                        detail: format!("`{part}` is not declared"),
                    });
                }
            }
        };
        for f in &c.fields {
            check(&f.ty, &f.name);
        }
        for me in &c.methods {
            check(&me.returns, &me.name);
            for p in &me.params {
                check(&p.ty, &me.name);
            }
        }
    }
}

/// **A value is copied, never pointed at.**
///
/// ⚠ **This is the rule with teeth, and it is what stops the miscategorisation recurring.** Without it,
/// `Ref<Shape>` type-checks — and it *was* in the manifest, on a field the design types as a bare
/// `Shape`. A `Ref<T>` to something with no identity is a pointer to a copy: two of them compare
/// unequal while meaning the same thing, and nothing about the declaration says so.
fn values_are_never_referenced(m: &Manifest, out: &mut Vec<Violation>) {
    let valued: BTreeSet<&str> = m
        .classes
        .iter()
        .filter(|c| matches!(c.kind(), Kind::Struct | Kind::Variant | Kind::Enum))
        .map(|c| c.short_name())
        .collect();

    for c in &m.classes {
        let mut check = |ty: &str, member: &str| {
            // Only the immediate argument of a `Ref<…>` or `Kind<…>` is a reference; `Array<Cost>`
            // is a list of values and is fine.
            for wrapper in ["Ref<", "Kind<"] {
                let mut rest = ty;
                while let Some(at) = rest.find(wrapper) {
                    rest = &rest[at + wrapper.len()..];
                    let inner: String = rest
                        .chars()
                        .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                        .collect();
                    if valued.contains(inner.as_str()) {
                        out.push(Violation {
                            rule: "value",
                            where_: format!("{}::{member}", c.path),
                            detail: format!(
                                "`{wrapper}{inner}>` — {inner} is a value; values are copied, never referenced. Use a bare `{inner}`."
                            ),
                        });
                    }
                }
            }
        };
        for f in &c.fields {
            check(&f.ty, &f.name);
        }
        for me in &c.methods {
            check(&me.returns, &me.name);
            for p in &me.params {
                check(&p.ty, &me.name);
            }
        }
    }
}

/// `Array<Ref<Actor>>` → `Array`, `Ref`, `Actor`.
fn split_type(ty: &str) -> Vec<String> {
    ty.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn inheritance_depth(m: &Manifest, out: &mut Vec<Violation>) {
    for c in &m.classes {
        if c.kind() != Kind::Object {
            continue;
        }
        match m.depth_of(c) {
            Some(d) if d > MAX_DEPTH => out.push(Violation {
                rule: "depth",
                where_: c.path.clone(),
                detail: format!("{d} hops from /Core/Object; the limit is {MAX_DEPTH}"),
            }),
            None if c.path != "/Core/Object" => out.push(Violation {
                rule: "depth",
                where_: c.path.clone(),
                detail: "does not reach /Core/Object through `extends`".into(),
            }),
            _ => {}
        }
    }
}

/// A `mutable` member that is not `exposed` is writable by nothing and readable by nobody: the
/// inspector shows only exposed members, so the mutability has no surface to act through.
fn mutable_requires_exposed(m: &Manifest, out: &mut Vec<Violation>) {
    for c in &m.classes {
        for f in &c.fields {
            if f.mutable && !f.exposed {
                out.push(Violation {
                    rule: "mutable",
                    where_: format!("{}::{}", c.path, f.name),
                    detail: "`mutable` requires `exposed`".into(),
                });
            }
        }
    }
}

fn no_wide_integers(m: &Manifest, out: &mut Vec<Violation>) {
    let mut flag = |c: &Class, member: &str, ty: &str| {
        for part in split_type(ty) {
            if BANNED_TYPES.contains(&part.as_str()) {
                out.push(Violation {
                    rule: "u64",
                    where_: format!("{}::{member}", c.path),
                    detail: format!("`{part}` corrupts silently in TypeScript above 2^53"),
                });
            }
        }
    };
    for c in &m.classes {
        for f in &c.fields {
            flag(c, &f.name, &f.ty);
        }
        for me in &c.methods {
            flag(c, &me.name, &me.returns);
            for p in &me.params {
                flag(c, &me.name, &p.ty);
            }
        }
    }
}

/// One name, one signature. Rust cannot overload and TypeScript can, so allowing it would produce
/// two divergent surfaces from one manifest.
fn no_overloads(m: &Manifest, out: &mut Vec<Violation>) {
    for c in &m.classes {
        let mut seen = BTreeSet::new();
        for name in c
            .fields
            .iter()
            .map(|f| f.name.as_str())
            .chain(c.methods.iter().map(|me| me.name.as_str()))
        {
            if !seen.insert(name) {
                out.push(Violation {
                    rule: "overload",
                    where_: format!("{}::{name}", c.path),
                    detail: "declared more than once in this class".into(),
                });
            }
        }
    }
}

/// Every declaration carries prose. The generator renders it into the reference, the inspector and
/// the palette tooltip, so an undocumented member is invisible in three places at once.
fn documented(m: &Manifest, out: &mut Vec<Violation>) {
    for c in &m.classes {
        if c.doc.trim().is_empty() {
            out.push(Violation {
                rule: "doc",
                where_: c.path.clone(),
                detail: "no `doc`".into(),
            });
        }
        for f in &c.fields {
            if f.doc.trim().is_empty() {
                out.push(Violation {
                    rule: "doc",
                    where_: format!("{}::{}", c.path, f.name),
                    detail: "no `doc`".into(),
                });
            }
        }
        for me in &c.methods {
            if me.doc.trim().is_empty() {
                out.push(Violation {
                    rule: "doc",
                    where_: format!("{}::{}", c.path, me.name),
                    detail: "no `doc`".into(),
                });
            }
        }
    }
}

/// Enums carry values and nothing else; structs and objects carry members and no values.
fn kind_consistency(m: &Manifest, out: &mut Vec<Violation>) {
    for c in &m.classes {
        match c.kind() {
            Kind::Enum => {
                if c.values.is_empty() {
                    out.push(Violation {
                        rule: "kind",
                        where_: c.path.clone(),
                        detail: "an enum with no values".into(),
                    });
                }
                if !c.fields.is_empty() || !c.methods.is_empty() {
                    out.push(Violation {
                        rule: "kind",
                        where_: c.path.clone(),
                        detail: "an enum may not carry fields or methods".into(),
                    });
                }
                if c.extends.is_some() {
                    out.push(Violation {
                        rule: "kind",
                        where_: c.path.clone(),
                        detail: "an enum may not extend".into(),
                    });
                }
            }
            Kind::Struct => {
                if !c.values.is_empty() {
                    out.push(Violation {
                        rule: "kind",
                        where_: c.path.clone(),
                        detail: "a struct may not carry enum values".into(),
                    });
                }
                if c.extends.is_some() {
                    out.push(Violation {
                        rule: "kind",
                        where_: c.path.clone(),
                        detail: "a struct may not extend — structs are copied, not subclassed"
                            .into(),
                    });
                }
            }
            Kind::Object => {
                if !c.values.is_empty() {
                    out.push(Violation {
                        rule: "kind",
                        where_: c.path.clone(),
                        detail: "an object may not carry enum values".into(),
                    });
                }
            }
            Kind::Variant => {
                if !c.values.is_empty() {
                    out.push(Violation {
                        rule: "kind",
                        where_: c.path.clone(),
                        detail: "a variant may not carry enum values — its forms are declarations                                  that extend it"
                            .into(),
                    });
                }
                // ⚠ A form must extend a *variant*. Letting one extend an object would give the
                // whole union identity by the back door, which is the confusion this kind exists to
                // end.
                if let Some(base) = &c.extends {
                    if m.get(base).map(|b| b.kind()) != Some(Kind::Variant) {
                        out.push(Violation {
                            rule: "kind",
                            where_: c.path.clone(),
                            detail: format!(
                                "a variant may only extend another variant, and {base} is not one"
                            ),
                        });
                    }
                }
            }
        }
        if c.status == Status::Deprecated && c.doc.trim().is_empty() {
            out.push(Violation {
                rule: "deprecated",
                where_: c.path.clone(),
                detail: "a deprecated declaration must say what replaces it".into(),
            });
        }
    }
}
