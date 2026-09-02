//! **Values** — what sits on the right of a `Key=`.
//!
//! # Typed references are self-describing, and the tag set is closed
//!
//! ⚠ **A reader and a validator know what a value is without a schema lookup**, which is the property
//! that lets a fragment travel through prose and lets third-party tooling exist. It only holds while the
//! tag set is **closed**: an unknown `Foo'…'` that parsed as *some reference* would make the
//! self-describing claim false for exactly the documents nobody checked.
//!
//! # A variant is a tuple naming its form, not a class reference
//!
//! ⚠ **`Shape=(Form=CubeShape,extents=(2.0,1.0,2.0))`, never `Kind'/Core/CubeShape'`.** Nothing is being
//! constructed, and two identical shapes are the **same value** rather than two instances that happen to
//! match. Writing it as a class reference would make a value look like an object, which is the confusion
//! the `Kind`/`Ref` split exists to end.

use std::fmt;

/// The closed set of reference tags.
///
/// ⚠ **Closed, and the error names what was written.** A parser that accepted an unlisted tag would turn
/// `Knid'/Core/Item'` into a silent default rather than a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefTag {
    /// A class — nothing is constructed.
    Kind,
    /// A live instance.
    Ref,
    /// A file on disk. ⚠ Only ever a *value*, never a type.
    Asset,
    /// A resource class. ⚠ Only ever a *type*, paired with an `Asset'…'` value.
    Resource,
    /// An enum, whose value follows.
    Enum,
    /// A value with alternative forms.
    Variant,
    /// A struct, by path.
    Struct,
    /// A homogeneous sequence, by element path.
    Array,
    /// A tag value.
    Tag,
}

impl RefTag {
    /// Every tag, in the order the format document lists them.
    pub const ALL: [RefTag; 9] = [
        RefTag::Kind,
        RefTag::Ref,
        RefTag::Asset,
        RefTag::Resource,
        RefTag::Enum,
        RefTag::Variant,
        RefTag::Struct,
        RefTag::Array,
        RefTag::Tag,
    ];

    /// Parse a tag name.
    pub fn from_name(name: &str) -> Option<RefTag> {
        RefTag::ALL.into_iter().find(|t| t.name() == name)
    }

    /// The spelling.
    pub fn name(self) -> &'static str {
        match self {
            RefTag::Kind => "Kind",
            RefTag::Ref => "Ref",
            RefTag::Asset => "Asset",
            RefTag::Resource => "Resource",
            RefTag::Enum => "Enum",
            RefTag::Variant => "Variant",
            RefTag::Struct => "Struct",
            RefTag::Array => "Array",
            RefTag::Tag => "Tag",
        }
    }

    /// May this tag appear in a `Type=` position?
    ///
    /// ⚠ **`Asset'…'` may not.** A file is a value; a *resource class* is the type that loads it, and
    /// the pair is two facts in two places. Conflating them is what makes lazy loading impossible to
    /// express.
    pub fn is_type_position(self) -> bool {
        self != RefTag::Asset
    }

    /// May this tag appear in a value position?
    ///
    /// ⚠ **`Resource'…'` may not.** It names a loader, and a loader is not a value.
    pub fn is_value_position(self) -> bool {
        self != RefTag::Resource
    }
}

impl fmt::Display for RefTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A parsed value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `30.0`, `-320`, `1`.
    ///
    /// ⚠ **One numeric form, carrying whether it was written with a point.** Two variants would make
    /// `Default=1` and `Default=1.0` different documents that mean the same thing, and the canonical
    /// writer's job is to make the same meaning produce the same bytes.
    Number { value: f64, fractional: bool },
    /// `"how far the rope reaches"`.
    Quoted(String),
    /// `PROGRESSION`, `true`, `float`, `core.branch`.
    Ident(String),
    /// `(-320,0)` or `(value=30.0,min=8.0)`.
    ///
    /// ⚠ **Positional and keyed entries in one variant.** `(a,b)` and `(k=v)` are the same production,
    /// and splitting them would force every consumer to handle two shapes of the same syntax.
    Tuple(Vec<(Option<String>, Value)>),
    /// `Kind'/Core/Item'`, `Tag'Enemy.Boss'`.
    Reference { tag: RefTag, path: String },
    /// `Array<Ref'/Core/Object'>`, `Map<String,int>`.
    Container { name: String, args: Vec<Value> },
    /// `Enum'/Core/NodeKind'.REACH` — a reference with a trailing selector.
    Member { base: Box<Value>, member: String },
    /// `Asset'/Content/x.cvunlock'#"Row"` — a reference into a row of a data resource.
    ///
    /// ⚠ **Distinct from [`Member`](Value::Member) on purpose.** A `.` selector picks a declared thing
    /// the type system knows about; a `#` selector picks a **row** whose existence only the file can
    /// confirm. Spelling them the same would hide which failures are compile-time.
    Row { base: Box<Value>, row: String },
}

impl Value {
    /// An integer.
    pub fn int(v: i64) -> Value {
        Value::Number {
            value: v as f64,
            fractional: false,
        }
    }

    /// A float.
    pub fn float(v: f64) -> Value {
        Value::Number {
            value: v,
            fractional: true,
        }
    }

    /// A reference.
    pub fn reference(tag: RefTag, path: impl Into<String>) -> Value {
        Value::Reference {
            tag,
            path: path.into(),
        }
    }

    /// The `Form=` of a variant tuple, if it has one.
    ///
    /// ⚠ **The one keyed entry a validator always looks for**, because a variant without it is a struct
    /// whose shape nobody can determine.
    pub fn form(&self) -> Option<&str> {
        let Value::Tuple(entries) = self else {
            return None;
        };
        entries.iter().find_map(|(k, v)| match (k.as_deref(), v) {
            (Some("Form"), Value::Ident(name)) => Some(name.as_str()),
            _ => None,
        })
    }

    /// Look a keyed entry up in a tuple.
    pub fn get(&self, key: &str) -> Option<&Value> {
        let Value::Tuple(entries) = self else {
            return None;
        };
        entries
            .iter()
            .find(|(k, _)| k.as_deref() == Some(key))
            .map(|(_, v)| v)
    }

    /// The reference tag, when this is one.
    pub fn tag(&self) -> Option<RefTag> {
        match self {
            Value::Reference { tag, .. } => Some(*tag),
            Value::Member { base, .. } | Value::Row { base, .. } => base.tag(),
            _ => None,
        }
    }
}

/// Formats a float with fixed decimals and never scientific notation.
///
/// ⚠ **`1e-7` and `0.0000001` are the same number and must be the same bytes.** Rust's default float
/// formatting switches representations by magnitude, which would make a canonical file depend on the
/// value rather than on the document.
pub fn canonical_float(v: f64) -> String {
    if !v.is_finite() {
        // ⚠ Not representable in the notation at all — better to write something a parser rejects
        // loudly than a token it would silently read as an identifier.
        return "NaN".to_string();
    }
    let mut s = format!("{v:.6}");
    while s.ends_with('0') && !s.ends_with(".0") {
        s.pop();
    }
    if s == "-0.0" {
        s = "0.0".to_string();
    }
    s
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number { value, fractional } => {
                if *fractional {
                    f.write_str(&canonical_float(*value))
                } else {
                    write!(f, "{}", *value as i64)
                }
            }
            Value::Quoted(s) => write!(f, "\"{s}\""),
            Value::Ident(s) => f.write_str(s),
            Value::Tuple(entries) => {
                f.write_str("(")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    if let Some(k) = k {
                        write!(f, "{k}=")?;
                    }
                    write!(f, "{v}")?;
                }
                f.write_str(")")
            }
            Value::Reference { tag, path } => write!(f, "{tag}'{path}'"),
            Value::Container { name, args } => {
                write!(f, "{name}<")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{a}")?;
                }
                f.write_str(">")
            }
            Value::Member { base, member } => write!(f, "{base}.{member}"),
            Value::Row { base, row } => write!(f, "{base}#\"{row}\""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tag_set_is_closed_and_an_unknown_one_does_not_resolve() {
        assert_eq!(RefTag::from_name("Kind"), Some(RefTag::Kind));
        assert_eq!(
            RefTag::from_name("Knid"),
            None,
            "a typo must not resolve to some reference"
        );
        assert_eq!(RefTag::ALL.len(), 9);
    }

    #[test]
    fn asset_is_a_value_only_and_resource_is_a_type_only() {
        // ⚠ The pair is two facts in two places; conflating them makes lazy loading inexpressible.
        assert!(!RefTag::Asset.is_type_position());
        assert!(RefTag::Asset.is_value_position());
        assert!(RefTag::Resource.is_type_position());
        assert!(!RefTag::Resource.is_value_position());
        assert!(RefTag::Kind.is_type_position() && RefTag::Kind.is_value_position());
    }

    #[test]
    fn a_variant_names_its_form() {
        let shape = Value::Tuple(vec![
            (Some("Form".into()), Value::Ident("CubeShape".into())),
            (
                Some("extents".into()),
                Value::Tuple(vec![
                    (None, Value::float(2.0)),
                    (None, Value::float(1.0)),
                    (None, Value::float(2.0)),
                ]),
            ),
        ]);
        assert_eq!(shape.form(), Some("CubeShape"));
        assert!(shape.get("extents").is_some());
        assert_eq!(Value::int(1).form(), None);
    }

    #[test]
    fn floats_are_fixed_decimal_and_never_scientific() {
        // ⚠ Rust switches representation by magnitude, which would make the bytes depend on the value.
        assert_eq!(canonical_float(30.0), "30.0");
        assert_eq!(canonical_float(0.5), "0.5");
        assert_eq!(canonical_float(-0.0), "0.0");
        assert!(!canonical_float(1e-7).contains('e'));
        assert!(!canonical_float(1e20).contains('e'));
    }

    #[test]
    fn an_integer_and_a_float_of_the_same_value_print_differently() {
        assert_eq!(Value::int(1).to_string(), "1");
        assert_eq!(Value::float(1.0).to_string(), "1.0");
    }

    #[test]
    fn a_member_selector_and_a_row_selector_are_different_values() {
        // ⚠ A `.` picks something the type system knows; a `#` picks a row only the file can confirm.
        let base = Value::reference(RefTag::Enum, "/Core/NodeKind");
        let m = Value::Member {
            base: Box::new(base.clone()),
            member: "REACH".into(),
        };
        let r = Value::Row {
            base: Box::new(Value::reference(RefTag::Asset, "/Content/x.cvunlock")),
            row: "Song".into(),
        };
        assert_ne!(m.to_string(), r.to_string());
        assert_eq!(m.to_string(), "Enum'/Core/NodeKind'.REACH");
        assert_eq!(r.to_string(), "Asset'/Content/x.cvunlock'#\"Song\"");
        assert_eq!(m.tag(), Some(RefTag::Enum));
        assert_eq!(r.tag(), Some(RefTag::Asset));
    }

    #[test]
    fn containers_use_angle_brackets_so_quotes_never_nest() {
        let v = Value::Container {
            name: "Array".into(),
            args: vec![Value::reference(RefTag::Ref, "/Core/Object")],
        };
        assert_eq!(v.to_string(), "Array<Ref'/Core/Object'>");
    }
}
