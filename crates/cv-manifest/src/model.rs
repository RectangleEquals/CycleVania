//! The shape of a tier-1 declaration.
//!
//! Deliberately plain data: the manifest is read once, validated, and handed to a generator. Nothing
//! here is clever, because everything here is the contract the whole toolchain is built on.

use std::collections::BTreeMap;

/// Which family a declaration belongs to.
///
/// The three are not interchangeable and the generator emits different things for each: an `Object`
/// has identity and may be subclassed, a `Struct` is copied and may not, an `Enum` is a closed value
/// set that becomes a dropdown in the palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Object,
    Struct,
    Enum,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "object" => Some(Kind::Object),
            "struct" => Some(Kind::Struct),
            "enum" => Some(Kind::Enum),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Object => "object",
            Kind::Struct => "struct",
            Kind::Enum => "enum",
        }
    }
}

/// Release state of a declaration or member.
///
/// Generated palettes ship only [`Status::Stable`]; docs render all three with badges. Keeping the
/// PROPOSED marker *in the manifest* rather than scattered through prose is what stops it being lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Status {
    Proposed,
    #[default]
    Stable,
    Deprecated,
}

impl Status {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "proposed" => Some(Status::Proposed),
            "stable" => Some(Status::Stable),
            "deprecated" => Some(Status::Deprecated),
            _ => None,
        }
    }
}

/// A parsed scalar. The schema uses only these four forms.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Bool(bool),
    Int(i64),
    /// An array of inline tables, used only for `args`.
    Params(Vec<Param>),
}

/// One argument of a [`Method`].
///
/// Note the absence of a `default` field: the binding contract forbids defaults **in an FFI
/// signature**, so the schema gives them nowhere to live. Pin defaults are a different thing and
/// belong to the member, not the argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: String,
}

/// A plain read of something already known. Becomes a pure get node.
#[derive(Clone, Debug, Default)]
pub struct Field {
    pub name: String,
    pub ty: String,
    pub api: bool,
    pub is_final: bool,
    pub exposed: bool,
    pub mutable: bool,
    pub status: Status,
    pub doc: String,
    /// The *behaviour* of the default, in prose — `"union of component collision"`, never the word
    /// "inherited". It is what the editor's OVERRIDES list shows.
    pub default: Option<String>,
}

/// Something that takes an argument, computes, or mutates. Becomes a call node with exec pins.
#[derive(Clone, Debug, Default)]
pub struct Method {
    pub name: String,
    pub params: Vec<Param>,
    pub returns: String,
    pub api: bool,
    pub is_final: bool,
    pub is_abstract: bool,
    /// A question the core asks. Drives a schematic's OVERRIDES list.
    pub hook: bool,
    pub status: Status,
    pub doc: String,
    pub default: Option<String>,
}

/// One member of an enum.
#[derive(Clone, Debug, Default)]
pub struct EnumValue {
    pub name: String,
    pub doc: String,
}

/// A tier-1 declaration.
#[derive(Clone, Debug, Default)]
pub struct Class {
    pub path: String,
    pub extends: Option<String>,
    pub kind: Option<Kind>,
    pub sealed: bool,
    pub is_abstract: bool,
    pub status: Status,
    pub doc: String,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pub values: Vec<EnumValue>,
}

impl Class {
    pub fn kind(&self) -> Kind {
        self.kind.unwrap_or(Kind::Object)
    }

    /// The last path segment — `Actor` for `/Core/Actor`.
    pub fn short_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

/// Everything declared, in file order.
///
/// File order is preserved because the generator's output must be byte-stable: reordering the
/// manifest would reorder every generated artifact and produce a diff that means nothing.
#[derive(Clone, Debug, Default)]
pub struct Manifest {
    pub version: i64,
    pub classes: Vec<Class>,
}

impl Manifest {
    pub fn by_path(&self) -> BTreeMap<&str, &Class> {
        self.classes.iter().map(|c| (c.path.as_str(), c)).collect()
    }

    pub fn get(&self, path: &str) -> Option<&Class> {
        self.classes.iter().find(|c| c.path == path)
    }

    pub fn count_of(&self, kind: Kind) -> usize {
        self.classes.iter().filter(|c| c.kind() == kind).count()
    }

    /// Every field and method across every class.
    pub fn member_count(&self) -> usize {
        self.classes
            .iter()
            .map(|c| c.fields.len() + c.methods.len())
            .sum()
    }

    /// Distance from `/Core/Object`, following `extends`. `None` if the chain does not reach it.
    pub fn depth_of(&self, class: &Class) -> Option<usize> {
        let mut depth = 0usize;
        let mut cur = class;
        loop {
            if cur.path == "/Core/Object" {
                return Some(depth);
            }
            let parent = cur.extends.as_deref()?;
            cur = self.get(parent)?;
            depth += 1;
            // A cycle in `extends` would otherwise spin forever; the validator reports it properly.
            if depth > self.classes.len() {
                return None;
            }
        }
    }
}
