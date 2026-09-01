//! **Mount-pointed paths** — `/Core/Item`, `/Content/Items/Hookshot`.
//!
//! The core requires these: a text token in a file has to resolve to one specific class,
//! unambiguously, forever.
//!
//! ⚠ **A bare name breaks the moment a project and a preset both define one.** Two `Door` classes and
//! nothing in the file says which — and the failure is silent, because both resolve to *something*.
//! Mounting the path is the whole fix, and it is why the format spends the extra characters.
//!
//! # Two mounts, and the boundary between them is a rule
//!
//! | Mount | Contains |
//! |---|---|
//! | `/Core/…` | tier-1 classes, implemented in Rust |
//! | `/Content/…` | authored schematics *and* assets |
//!
//! Content may extend Core. Core may never extend Content — which is what keeps the tier-1 surface
//! something a project can rely on rather than something a project can move under itself.
//!
//! # `Resource'…'` and `Asset'…'` are two halves of one reference
//!
//! ⚠ **Never interchangeable.** A class never appears in a value position and a path never appears in
//! a type position. A resource reference is therefore two facts carried in two places, exactly as a
//! `TSoftObjectPtr<UStaticMesh>` is — and the pair is what lets the core resolve the path with *that
//! class's own loader* rather than guessing a format from an extension.

use std::fmt;

/// Which mount a path is rooted at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mount {
    /// Tier-1 classes, implemented in Rust.
    Core,
    /// Authored schematics and assets.
    Content,
}

impl Mount {
    /// The leading segment, without slashes.
    pub fn as_str(self) -> &'static str {
        match self {
            Mount::Core => "Core",
            Mount::Content => "Content",
        }
    }

    /// May something in this mount extend something in `base`?
    ///
    /// ⚠ **Content extends Core; Core never extends Content.** Allowing the reverse would let a
    /// project move the tier-1 surface under itself, and every guarantee stated about `/Core/…` would
    /// become a guarantee about whatever the project last edited.
    pub fn may_extend(self, base: Mount) -> bool {
        matches!(
            (self, base),
            (Mount::Core, Mount::Core) | (Mount::Content, _)
        )
    }
}

impl fmt::Display for Mount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What is wrong with a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    /// It did not start with `/`.
    NotRooted { text: String },
    /// The first segment was not a known mount.
    UnknownMount { text: String, mount: String },
    /// There was nothing after the mount.
    NoClass { text: String },
    /// A segment was empty, or held something other than `[A-Za-z0-9_]`.
    BadSegment { text: String, segment: String },
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::NotRooted { text } => {
                write!(f, "{text:?} is not rooted — a class path starts with `/`")
            }
            PathError::UnknownMount { text, mount } => write!(
                f,
                "{text:?} is mounted at {mount:?}; the mounts are /Core and /Content"
            ),
            PathError::NoClass { text } => {
                write!(f, "{text:?} names a mount but no class inside it")
            }
            PathError::BadSegment { text, segment } => write!(
                f,
                "{text:?} has the segment {segment:?}; segments are [A-Za-z0-9_] and never empty"
            ),
        }
    }
}

impl std::error::Error for PathError {}

/// A mount-pointed path to a class — `/Core/Item`, `/Content/Items/Hookshot`.
///
/// ⚠ **The path is the id in a table, not a module system.** A move rewrites one row; it does not
/// reorganise anything, and nothing about the path implies where the file lives or what may see it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassPath {
    text: String,
    mount: Mount,
}

impl ClassPath {
    /// Parse a mount-pointed path.
    pub fn new(text: &str) -> Result<Self, PathError> {
        let owned = text.to_string();
        let rest = text.strip_prefix('/').ok_or(PathError::NotRooted {
            text: owned.clone(),
        })?;
        let mut segments = rest.split('/');
        let mount_text = segments.next().unwrap_or_default();
        let mount = match mount_text {
            "Core" => Mount::Core,
            "Content" => Mount::Content,
            other => {
                return Err(PathError::UnknownMount {
                    text: owned,
                    mount: other.to_string(),
                })
            }
        };
        let tail: Vec<&str> = segments.collect();
        if tail.is_empty() {
            return Err(PathError::NoClass { text: owned });
        }
        for segment in &tail {
            if segment.is_empty()
                || !segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(PathError::BadSegment {
                    text: owned,
                    segment: segment.to_string(),
                });
            }
        }
        Ok(ClassPath { text: owned, mount })
    }

    /// A `/Core/…` path, for the tier-1 classes named in Rust.
    ///
    /// # Panics
    ///
    /// On an invalid path. Only for literals the compiler can see — never for parsed input.
    pub fn core(text: &str) -> Self {
        ClassPath::new(text).expect("a valid /Core path literal")
    }

    /// The full text, mount included.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Which mount it is rooted at.
    pub fn mount(&self) -> Mount {
        self.mount
    }

    /// The segments after the mount.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.text.split('/').skip(2)
    }

    /// The last segment — the class's own name.
    pub fn leaf(&self) -> &str {
        self.text.rsplit('/').next().unwrap_or_default()
    }

    /// The path with the last segment removed, or `None` at the mount root.
    ///
    /// ⚠ **This is folder nesting, not inheritance.** `/Content/Items/Hookshot` sitting under
    /// `/Content/Items` says where a developer filed it and nothing about what it extends — ancestry
    /// is a [`crate::class::ClassRegistry`] question, and conflating the two would make moving a file
    /// change what its class *is*.
    pub fn folder(&self) -> Option<ClassPath> {
        let cut = self.text.rfind('/')?;
        ClassPath::new(&self.text[..cut]).ok()
    }
}

impl fmt::Display for ClassPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// A path to a **file on disk** — `Asset'/Content/Meshes/hookshot.glb'`.
///
/// ⚠ **Only ever a value, never a type.** The type half is a `Resource'…'` naming the resource
/// *class*, and the core loads this path with that class's own loader. An asset path alone says
/// nothing about how to read the bytes, which is why guessing from the extension is not the design.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPath {
    text: String,
}

impl AssetPath {
    /// Parse an asset path. Must be under `/Content` — core owns no files.
    ///
    /// ⚠ Segments may carry a `.`, because the leaf is a filename. That is the one difference from a
    /// [`ClassPath`], and it is also why the two are separate types rather than one with a flag: a
    /// class named `hookshot.glb` and a file named `Hookshot` are both nonsense, and only distinct
    /// types make them both unrepresentable.
    pub fn new(text: &str) -> Result<Self, PathError> {
        let owned = text.to_string();
        let rest = text.strip_prefix('/').ok_or(PathError::NotRooted {
            text: owned.clone(),
        })?;
        let mut segments = rest.split('/');
        match segments.next().unwrap_or_default() {
            "Content" => {}
            other => {
                return Err(PathError::UnknownMount {
                    text: owned,
                    mount: other.to_string(),
                })
            }
        }
        let tail: Vec<&str> = segments.collect();
        if tail.is_empty() {
            return Err(PathError::NoClass { text: owned });
        }
        for segment in &tail {
            if segment.is_empty()
                || !segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
            {
                return Err(PathError::BadSegment {
                    text: owned,
                    segment: segment.to_string(),
                });
            }
        }
        Ok(AssetPath { text: owned })
    }

    /// The full text.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The filename.
    pub fn file_name(&self) -> &str {
        self.text.rsplit('/').next().unwrap_or_default()
    }

    /// The extension, lowercased, without the dot.
    pub fn extension(&self) -> Option<String> {
        let name = self.file_name();
        let dot = name.rfind('.')?;
        Some(name[dot + 1..].to_ascii_lowercase())
    }
}

impl fmt::Display for AssetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl crate::serialize::Serialize for ClassPath {
    fn serialize(&self, w: &mut crate::serialize::Writer) {
        w.str(&self.text);
    }
}

impl crate::serialize::Deserialize for ClassPath {
    /// ⚠ **Reconstructed without re-parsing on the wire**, deliberately: a path that was valid when
    /// written is valid when read, and re-running the parse would turn a byte-level corruption into a
    /// *parse* error that names the wrong cause. Bound checking against the manifest is a separate
    /// load-time pass — see [`crate::class::PinType::accepts`].
    fn deserialize(r: &mut crate::serialize::Reader<'_>) -> crate::serialize::SerResult<Self> {
        let text = r.str()?;
        ClassPath::new(&text)
            .map_err(|_| crate::serialize::SerError::InvalidValue("malformed class path"))
    }
}

impl crate::serialize::Serialize for AssetPath {
    fn serialize(&self, w: &mut crate::serialize::Writer) {
        w.str(&self.text);
    }
}

impl crate::serialize::Deserialize for AssetPath {
    fn deserialize(r: &mut crate::serialize::Reader<'_>) -> crate::serialize::SerResult<Self> {
        let text = r.str()?;
        AssetPath::new(&text)
            .map_err(|_| crate::serialize::SerError::InvalidValue("malformed asset path"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_names_one_class_forever() {
        let p = ClassPath::new("/Content/Items/Hookshot").unwrap();
        assert_eq!(p.mount(), Mount::Content);
        assert_eq!(p.leaf(), "Hookshot");
        assert_eq!(p.segments().collect::<Vec<_>>(), vec!["Items", "Hookshot"]);
    }

    #[test]
    fn a_bare_name_is_refused_rather_than_resolved_to_something() {
        // ⚠ The failure the mount exists to prevent: a project and a preset both defining `Door`, and
        // nothing in the file saying which. Both would resolve — that is what makes it silent.
        assert!(matches!(
            ClassPath::new("Door"),
            Err(PathError::NotRooted { .. })
        ));
        assert!(matches!(
            ClassPath::new("/Items/Door"),
            Err(PathError::UnknownMount { .. })
        ));
    }

    #[test]
    fn a_mount_with_nothing_in_it_is_not_a_class() {
        assert!(matches!(
            ClassPath::new("/Core"),
            Err(PathError::NoClass { .. })
        ));
        assert!(matches!(
            ClassPath::new("/Content/"),
            Err(PathError::BadSegment { .. })
        ));
    }

    #[test]
    fn content_extends_core_and_core_never_extends_content() {
        // ⚠ The direction that keeps the tier-1 surface something a project can rely on rather than
        // something a project can move under itself.
        assert!(Mount::Content.may_extend(Mount::Core));
        assert!(Mount::Content.may_extend(Mount::Content));
        assert!(Mount::Core.may_extend(Mount::Core));
        assert!(!Mount::Core.may_extend(Mount::Content));
    }

    #[test]
    fn a_folder_is_not_an_ancestor() {
        // ⚠ Where a developer filed something says nothing about what it extends. Conflating the two
        // would make moving a file change what its class *is*.
        let p = ClassPath::new("/Content/Items/Hookshot").unwrap();
        assert_eq!(
            p.folder(),
            Some(ClassPath::new("/Content/Items").unwrap()),
            "folder nesting is a display concern"
        );
        assert_eq!(ClassPath::new("/Core/Item").unwrap().folder(), None);
    }

    #[test]
    fn an_asset_is_a_file_and_a_class_is_not() {
        // ⚠ The two are separate types so that a class named `hookshot.glb` and a file named
        // `Hookshot` are both unrepresentable rather than merely discouraged.
        let a = AssetPath::new("/Content/Meshes/hookshot.glb").unwrap();
        assert_eq!(a.file_name(), "hookshot.glb");
        assert_eq!(a.extension().as_deref(), Some("glb"));
        assert!(
            ClassPath::new("/Content/Meshes/hookshot.glb").is_err(),
            "a dot is not legal in a class segment"
        );
    }

    #[test]
    fn core_owns_no_files() {
        assert!(matches!(
            AssetPath::new("/Core/Meshes/thing.glb"),
            Err(PathError::UnknownMount { .. })
        ));
    }

    #[test]
    fn paths_round_trip_on_the_wire() {
        use crate::serialize::{from_bytes, to_bytes};
        let c = ClassPath::new("/Content/Items/Hookshot").unwrap();
        assert_eq!(from_bytes::<ClassPath>(&to_bytes(&c)).unwrap(), c);
        let a = AssetPath::new("/Content/Meshes/hookshot.glb").unwrap();
        assert_eq!(from_bytes::<AssetPath>(&to_bytes(&a)).unwrap(), a);
    }

    #[test]
    fn paths_order_and_hash_by_text_so_a_table_can_key_on_them() {
        let mut v = [
            ClassPath::core("/Core/Item"),
            ClassPath::core("/Core/Actor"),
            ClassPath::new("/Content/Items/Hookshot").unwrap(),
        ];
        v.sort();
        assert_eq!(v[0].as_str(), "/Content/Items/Hookshot");
        assert_eq!(v[1].as_str(), "/Core/Actor");
    }
}
