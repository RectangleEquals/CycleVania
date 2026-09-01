//! `crates/cv-bindings/index.d.ts` — the TypeScript surface.
//!
//! ⚠ Two mappings carry design weight and are worth stating rather than inferring:
//!
//! * **`ObjectId` becomes `string`**, never `number`. A JavaScript integer is exact only below 2^53,
//!   and ids are content-derived hashes, so a numeric id would corrupt for ~99.95% of content the
//!   moment it crossed the seam.
//! * **`Kind<T>` and `Ref<T>` become distinct branded types.** They are both a path at runtime, and
//!   letting TypeScript treat them as interchangeable would erase the class-versus-instance
//!   distinction that replaced seven retracted language features.

use cv_manifest::model::{Class, Kind, Status};
use cv_manifest::Manifest;
use std::fmt::Write;

pub fn emit(m: &Manifest) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "{}\n", crate::banner("//"));
    let _ = write!(s, "{PRELUDE}");

    for c in &m.classes {
        if c.status == Status::Deprecated {
            continue;
        }
        match c.kind() {
            Kind::Enum => emit_enum(&mut s, c),
            Kind::Variant => emit_variant(&mut s, c, m),
            _ => emit_interface(&mut s, c),
        }
    }
    s
}

/// A value with alternative forms, as a **discriminated union**.
///
/// ⚠ **The `form` literal is the whole point.** Without a discriminant TypeScript cannot narrow, so a
/// developer holding a `Shape` could read `.radius` off a cube and be told nothing. With it, `switch
/// (s.form)` narrows in each arm and an unhandled form is a compile error at the `never` check.
///
/// A form's own fields are emitted by [`emit_interface`] when its turn comes, so this writes only the
/// union — except for the base's shared members, which every form carries.
fn emit_variant(s: &mut String, c: &Class, m: &Manifest) {
    let forms: Vec<&Class> = m
        .classes
        .iter()
        .filter(|f| f.extends.as_deref() == Some(c.path.as_str()) && f.status != Status::Deprecated)
        .collect();

    // The base carries the shared members. A form extends it, so each arm inherits them.
    emit_interface(s, c);

    if forms.is_empty() {
        return;
    }
    let _ = writeln!(s, "/**");
    let _ = writeln!(
        s,
        " * The forms of {}. Switch on `form` — TypeScript narrows each arm.",
        c.short_name()
    );
    let _ = writeln!(s, " */");
    let _ = writeln!(s, "export type {}Form =", c.short_name());
    for (i, f) in forms.iter().enumerate() {
        let sep = if i + 1 == forms.len() { ";" } else { "" };
        let _ = writeln!(
            s,
            "  | ({} & {{ form: {:?} }}){sep}",
            f.short_name(),
            f.short_name()
        );
    }
    let _ = writeln!(s);
}

fn emit_enum(s: &mut String, c: &Class) {
    let _ = writeln!(s, "/** {} */", one_line(&c.doc));
    let _ = writeln!(s, "export type {} =", c.short_name());
    for (i, v) in c.values.iter().enumerate() {
        let sep = if i + 1 == c.values.len() { ";" } else { "" };
        if v.doc.is_empty() {
            let _ = writeln!(s, "  | {:?}{sep}", v.name);
        } else {
            let _ = writeln!(s, "  /** {} */", one_line(&v.doc));
            let _ = writeln!(s, "  | {:?}{sep}", v.name);
        }
    }
    let _ = writeln!(s);
}

fn emit_interface(s: &mut String, c: &Class) {
    let _ = writeln!(s, "/**");
    for line in wrap(&c.doc, 96) {
        let _ = writeln!(s, " * {line}");
    }
    if c.sealed {
        let _ = writeln!(s, " *");
        let _ = writeln!(s, " * Sealed: content may not subclass this.");
    }
    let _ = writeln!(s, " */");

    let ext = match &c.extends {
        Some(p) => format!(" extends {}", p.rsplit('/').next().unwrap_or(p)),
        None => String::new(),
    };
    let _ = writeln!(s, "export interface {}{ext} {{", c.short_name());

    for f in c
        .fields
        .iter()
        .filter(|f| f.api && f.status != Status::Deprecated)
    {
        let _ = writeln!(s, "  /**");
        for line in wrap(&f.doc, 92) {
            let _ = writeln!(s, "   * {line}");
        }
        if let Some(d) = &f.default {
            let _ = writeln!(s, "   * @default {d}");
        }
        if f.status == Status::Proposed {
            let _ = writeln!(s, "   * @experimental PROPOSED — may change or be removed.");
        }
        let _ = writeln!(s, "   */");
        let ro = if f.mutable { "" } else { "readonly " };
        let _ = writeln!(s, "  {ro}{}: {};", f.name, ts_type(&f.ty));
    }

    for me in c
        .methods
        .iter()
        .filter(|m| m.api && m.status != Status::Deprecated)
    {
        let _ = writeln!(s, "  /**");
        for line in wrap(&me.doc, 92) {
            let _ = writeln!(s, "   * {line}");
        }
        if let Some(d) = &me.default {
            let _ = writeln!(s, "   * @default {d}");
        }
        if me.hook {
            let _ = writeln!(s, "   * @remarks A hook — a question the core asks.");
        }
        if me.status == Status::Proposed {
            let _ = writeln!(s, "   * @experimental PROPOSED — may change or be removed.");
        }
        let _ = writeln!(s, "   */");
        let args: Vec<String> = me
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, ts_type(&p.ty)))
            .collect();
        let _ = writeln!(
            s,
            "  {}({}): {};",
            me.name,
            args.join(", "),
            ts_type(&me.returns)
        );
    }

    let _ = writeln!(s, "}}\n");
}

/// Map a manifest type expression onto TypeScript.
fn ts_type(ty: &str) -> String {
    if let Some(inner) = shell(ty, "Array") {
        return format!("{}[]", ts_type(inner));
    }
    if let Some(inner) = shell(ty, "Map") {
        let (k, v) = inner.split_once(',').unwrap_or((inner, "unknown"));
        return format!("Record<{}, {}>", ts_type(k.trim()), ts_type(v.trim()));
    }
    for (shell_name, brand) in [
        ("Kind", "ClassPath"),
        ("Ref", "InstanceRef"),
        ("Resource", "AssetRef"),
    ] {
        if let Some(inner) = shell(ty, shell_name) {
            return format!("{brand}<{}>", ts_type(inner));
        }
    }
    match ty {
        "bool" => "boolean".into(),
        "int" | "float" => "number".into(),
        "String" => "string".into(),
        "void" => "void".into(),
        // Never `number`: content-derived ids are hashes, and a JS integer is exact only below 2^53.
        "ObjectId" => "string".into(),
        other => other.into(),
    }
}

fn shell<'a>(ty: &'a str, name: &str) -> Option<&'a str> {
    ty.strip_prefix(name)
        .and_then(|r| r.strip_prefix('<'))
        .and_then(|r| r.strip_suffix('>'))
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

const PRELUDE: &str = r#"
/**
 * A picked CLASS path — `Kind<T>` in the manifest.
 *
 * Branded so TypeScript keeps it distinct from an instance reference. Both are a path at runtime,
 * and letting them mix would erase the class-versus-instance distinction the whole authoring model
 * rests on.
 */
export type ClassPath<T> = string & { readonly __class?: T };

/** A live INSTANCE reference — `Ref<T>` in the manifest. */
export type InstanceRef<T> = string & { readonly __instance?: T };

/** A FILE on disk — `Resource<T>` in the manifest, paired with an asset path. */
export type AssetRef<T> = string & { readonly __asset?: T };

/** Metadata values. A closed set: anything outside it does not survive the seam. */
export type MetaValue =
  | boolean
  | number
  | string
  | Vec3
  | Transform
  | MetaValue[]
  | Record<string, MetaValue>;

"#;
