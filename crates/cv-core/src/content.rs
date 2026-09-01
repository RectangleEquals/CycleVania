//! The content registry — everything a host has declared that generation may draw on (L0).
//!
//! # What "content" is
//!
//! A host does not hand the generator *instances*; it declares **kinds of thing that may exist**: a
//! heavy door, a bronze key, a blink-dash unlock, a relay puzzle. L0 turns those declarations
//! into a registry, and every later layer draws from it. The registry is therefore the complete answer
//! to "what could this world contain?", which is exactly why it is a fingerprint input (see
//! [`crate::fingerprint`]): change what content exists and you have changed the recipe.
//!
//! # Identity is content-addressed
//!
//! Registered content uses [`ObjectId::derived`], not a sequential id. That matters: a door's identity
//! should follow *what it is* (`("actor", "crawler/door_heavy")`), not the order it happened to be
//! registered in. Two builds that declare the same content agree on its id without coordinating, and
//! inserting a new piece of content does not renumber everything after it.
//!
//! # Determinism
//!
//! The index is a `BTreeMap`, never a `HashMap`: iteration order is by id and therefore identical on
//! every run and target. A `HashMap` here would silently reorder the fingerprint's inputs.

use crate::object::{Object, ObjectHeader, ObjectId};
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use std::collections::BTreeMap;
use std::fmt;

/// What sort of thing a piece of registered content is.
///
/// The distinction that earns its place here is [`ContentKind::is_schedulable`] — whether L1 may
/// *place or bias* it. Everything else about a kind is the host's business.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentKind {
    // --- schedulable: the algorithm places or biases these ---
    /// A world-placeable unit.
    Actor,
    /// An obtainable Actor carrying a classification and optional grant.
    Item,
    /// A stateful Actor solved as a state graph.
    Puzzle,

    // --- not schedulable: referenced or composed, never placed on their own ---
    /// The project's progression vocabulary — an `UnlockTableResource`.
    ///
    /// ⚠ **The file is content; an atom is not.** A table is referenced by `grants` and by
    /// `HoldsRule`, never placed, so it is not schedulable and fills no scope. The atoms inside it
    /// are `Unlock` rows, which are data rather than registered content.
    UnlockTable,
    /// A reusable concern attached to an Actor.
    Component,
    /// A triggered consequence.
    Action,
    /// A parametric geometry primitive.
    Shape,
    /// A material/interaction property assignable to a surface.
    SurfaceProperty,
    /// Disk-backed mesh data (metadata only in the output; the host loads real geometry).
    StaticMesh,
    /// A disk-backed curve.
    CurveTable,
    /// An **opt-in** macro-structure template constraining L2's topology.
    ///
    /// Not schedulable: a spine is not *placed*, it shapes where placement happens.
    Spine,
}

impl ContentKind {
    /// Every kind, in declaration order — the canonical order for iteration and tags.
    pub const ALL: [ContentKind; 11] = [
        ContentKind::Actor,
        ContentKind::Item,
        ContentKind::Puzzle,
        ContentKind::UnlockTable,
        ContentKind::Component,
        ContentKind::Action,
        ContentKind::Shape,
        ContentKind::SurfaceProperty,
        ContentKind::StaticMesh,
        ContentKind::CurveTable,
        ContentKind::Spine,
    ];

    /// May L1 place or bias this?
    ///
    /// The compiler and the editor answer "can I schedule this?" from here rather than guessing, so a
    /// dev never has to discover by trial that a `Component` cannot be scheduled on its own.
    pub fn is_schedulable(self) -> bool {
        matches!(
            self,
            ContentKind::Actor | ContentKind::Item | ContentKind::Puzzle
        )
    }

    /// The scope kinds this content naturally fills.
    ///
    /// Without this, a slot would count content among its available variety that could never go
    /// there, and inflate its adaptive target by the difference.
    ///
    /// ⚠ **Every *schedulable* kind currently fills the same scopes.** The two that once differed are
    /// both gone: `Biome` at Area became dial values on a spine slot, and `Token` at Space became an
    /// `Unlock` table row, which is data rather than placed content. What this still separates is
    /// **placeable from referenced**, which is what keeps a `Component` or an `UnlockTable` out of a
    /// room's variety count. A schedule may override it.
    pub fn default_scopes(self) -> &'static [crate::node::NodeKind] {
        use crate::node::NodeKind::*;
        match self {
            ContentKind::Actor | ContentKind::Item | ContentKind::Puzzle => &[Space, Spatial],
            // Not schedulable: referenced or composed, never placed on their own.
            ContentKind::UnlockTable
            | ContentKind::Component
            | ContentKind::Action
            | ContentKind::Shape
            | ContentKind::SurfaceProperty
            | ContentKind::StaticMesh
            | ContentKind::CurveTable
            | ContentKind::Spine => &[],
        }
    }

    /// The namespace used when deriving ids for this kind, so an Actor and an Item may share a path
    /// without colliding.
    pub fn namespace(self) -> &'static str {
        match self {
            ContentKind::Actor => "actor",
            ContentKind::Item => "item",
            ContentKind::Puzzle => "puzzle",
            ContentKind::UnlockTable => "unlock_table",
            ContentKind::Component => "component",
            ContentKind::Action => "action",
            ContentKind::Shape => "shape",
            ContentKind::SurfaceProperty => "surface",
            ContentKind::StaticMesh => "mesh",
            ContentKind::CurveTable => "curve",
            ContentKind::Spine => "spine",
        }
    }

    fn tag(self) -> u8 {
        ContentKind::ALL
            .iter()
            .position(|k| *k == self)
            .unwrap_or(0) as u8
    }

    fn from_tag(tag: u8) -> Option<Self> {
        ContentKind::ALL.get(tag as usize).copied()
    }
}

impl fmt::Display for ContentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.namespace())
    }
}

/// One registered piece of content.
///
/// ⚠ `PartialEq` but not `Eq`: the header carries metadata, which may hold a float.
#[derive(Clone, Debug, PartialEq)]
pub struct ContentEntry {
    header: ObjectHeader,
    kind: ContentKind,
    /// The dev-facing path this was registered under (`"crawler/door_heavy"`) — the id's preimage, and
    /// what a diagnostic should show a human.
    path: String,
    /// A digest of whatever *defines* this content: its compiled script, its asset bytes, or its
    /// declaration. Two builds whose content differs only in behaviour still get different
    /// fingerprints because of this.
    source_digest: u64,
}

impl ContentEntry {
    /// What sort of content this is.
    pub fn kind(&self) -> ContentKind {
        self.kind
    }

    /// The path it was registered under.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The digest of its defining source.
    pub fn source_digest(&self) -> u64 {
        self.source_digest
    }

    /// May L1 place or bias this?
    pub fn is_schedulable(&self) -> bool {
        self.kind.is_schedulable()
    }
}

impl Object for ContentEntry {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut ObjectHeader {
        &mut self.header
    }
    fn type_name(&self) -> &'static str {
        self.kind.namespace()
    }
}

/// What can go wrong while registering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// Two pieces of content derived the same id — the same kind and path registered twice.
    DuplicateContent { kind: ContentKind, path: String },
    /// No content is registered under that id.
    Unknown { id: ObjectId },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::DuplicateContent { kind, path } => {
                write!(f, "{kind} content {path:?} is already registered")
            }
            RegistryError::Unknown { id } => write!(f, "no content registered as {id}"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// The set of content a world may be built from.
///
/// Ordered by [`ObjectId`] throughout, so iteration — and therefore the fingerprint — does not depend
/// on registration order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContentRegistry {
    entries: BTreeMap<ObjectId, ContentEntry>,
}

impl ContentRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        ContentRegistry {
            entries: BTreeMap::new(),
        }
    }

    /// How many pieces of content are registered.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Is nothing registered?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Register content, returning its content-addressed id.
    ///
    /// The id is derived from `(kind.namespace(), path)`, so registering the same kind and path twice
    /// is a duplicate rather than two entries that happen to look alike.
    pub fn register(
        &mut self,
        kind: ContentKind,
        path: impl Into<String>,
        source_digest: u64,
    ) -> Result<ObjectId, RegistryError> {
        let path = path.into();
        let id = ObjectId::derived(kind.namespace(), &path);
        if self.entries.contains_key(&id) {
            return Err(RegistryError::DuplicateContent { kind, path });
        }
        // The display name defaults to the last path segment; hosts may rename without changing identity.
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        self.entries.insert(
            id,
            ContentEntry {
                header: ObjectHeader::new(id, name),
                kind,
                path,
                source_digest,
            },
        );
        Ok(id)
    }

    /// The id content *would* get, without registering it — for looking up by path.
    pub fn id_for(kind: ContentKind, path: &str) -> ObjectId {
        ObjectId::derived(kind.namespace(), path)
    }

    /// Look up registered content.
    pub fn get(&self, id: ObjectId) -> Option<&ContentEntry> {
        self.entries.get(&id)
    }

    /// Look up registered content, erroring when absent.
    pub fn entry(&self, id: ObjectId) -> Result<&ContentEntry, RegistryError> {
        self.entries.get(&id).ok_or(RegistryError::Unknown { id })
    }

    /// Look up by kind and path.
    pub fn find(&self, kind: ContentKind, path: &str) -> Option<&ContentEntry> {
        self.get(ContentRegistry::id_for(kind, path))
    }

    /// Is this id registered?
    pub fn contains(&self, id: ObjectId) -> bool {
        self.entries.contains_key(&id)
    }

    /// Every entry, in id order — deterministic.
    pub fn iter(&self) -> impl Iterator<Item = (ObjectId, &ContentEntry)> + '_ {
        self.entries.iter().map(|(id, e)| (*id, e))
    }

    /// Every entry of a kind, in id order.
    pub fn of_kind(
        &self,
        kind: ContentKind,
    ) -> impl Iterator<Item = (ObjectId, &ContentEntry)> + '_ {
        self.iter().filter(move |(_, e)| e.kind == kind)
    }

    /// Everything L1 may place or bias, in id order — the eligible set the scheduler starts from.
    pub fn schedulable(&self) -> impl Iterator<Item = (ObjectId, &ContentEntry)> + '_ {
        self.iter().filter(|(_, e)| e.is_schedulable())
    }
}

impl Serialize for ContentKind {
    fn serialize(&self, w: &mut Writer) {
        w.u8(self.tag());
    }
}

impl Deserialize for ContentKind {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        ContentKind::from_tag(r.u8()?).ok_or(SerError::InvalidValue("unknown ContentKind tag"))
    }
}

impl Serialize for ContentEntry {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.header);
        w.write(&self.kind);
        w.str(&self.path);
        w.u64(self.source_digest);
    }
}

impl Deserialize for ContentEntry {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(ContentEntry {
            header: r.read()?,
            kind: r.read()?,
            path: r.str()?,
            source_digest: r.u64()?,
        })
    }
}

impl Serialize for ContentRegistry {
    fn serialize(&self, w: &mut Writer) {
        w.len(self.entries.len());
        // BTreeMap iterates in id order, so this byte sequence is canonical.
        for (id, entry) in &self.entries {
            w.write(id);
            w.write(entry);
        }
    }
}

impl Deserialize for ContentRegistry {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let n = r.u32()? as usize;
        let mut entries = BTreeMap::new();
        for _ in 0..n {
            let id: ObjectId = r.read()?;
            let entry: ContentEntry = r.read()?;
            if entries.insert(id, entry).is_some() {
                return Err(SerError::InvalidValue("duplicate content id in registry"));
            }
        }
        Ok(ContentRegistry { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::{from_bytes, to_bytes};

    fn populated() -> ContentRegistry {
        let mut r = ContentRegistry::new();
        r.register(ContentKind::Actor, "crawler/door_heavy", 0x11)
            .unwrap();
        r.register(ContentKind::Item, "crawler/key_bronze", 0x22)
            .unwrap();
        r.register(ContentKind::UnlockTable, "unlocks/core", 0x33)
            .unwrap();
        r.register(ContentKind::Component, "hinge", 0x44).unwrap();
        r.register(ContentKind::StaticMesh, "kit/door_a", 0x55)
            .unwrap();
        r
    }

    #[test]
    fn registration_is_content_addressed() {
        let mut a = ContentRegistry::new();
        let mut b = ContentRegistry::new();
        // Registered in opposite orders...
        let a1 = a.register(ContentKind::Actor, "door", 1).unwrap();
        let a2 = a.register(ContentKind::Item, "key", 2).unwrap();
        let b2 = b.register(ContentKind::Item, "key", 2).unwrap();
        let b1 = b.register(ContentKind::Actor, "door", 1).unwrap();
        // ...yields identical ids, and identical registries.
        assert_eq!(a1, b1);
        assert_eq!(a2, b2);
        assert_eq!(a, b, "registration order must not affect the registry");
    }

    #[test]
    fn kind_namespaces_prevent_path_collisions() {
        let mut r = ContentRegistry::new();
        let actor = r.register(ContentKind::Actor, "door", 1).unwrap();
        let item = r.register(ContentKind::Item, "door", 1).unwrap();
        assert_ne!(actor, item, "same path, different kind, must not collide");
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn duplicates_are_rejected() {
        let mut r = ContentRegistry::new();
        r.register(ContentKind::Actor, "door", 1).unwrap();
        assert_eq!(
            r.register(ContentKind::Actor, "door", 999),
            Err(RegistryError::DuplicateContent {
                kind: ContentKind::Actor,
                path: "door".into()
            })
        );
        assert_eq!(r.len(), 1, "a rejected registration must change nothing");
    }

    #[test]
    fn lookup_by_path_and_id() {
        let r = populated();
        let id = ContentRegistry::id_for(ContentKind::Item, "crawler/key_bronze");
        assert!(r.contains(id));
        assert_eq!(r.entry(id).unwrap().path(), "crawler/key_bronze");
        assert_eq!(
            r.entry(id).unwrap().name(),
            "key_bronze",
            "name defaults to the last segment"
        );
        assert_eq!(
            r.find(ContentKind::Item, "crawler/key_bronze")
                .unwrap()
                .source_digest(),
            0x22
        );
        assert!(r.find(ContentKind::Item, "nope").is_none());
        assert!(matches!(
            r.entry(ObjectId::from_raw(1)),
            Err(RegistryError::Unknown { .. })
        ));
    }

    #[test]
    fn the_schedulable_gate_is_explicit() {
        // Placeable things are schedulable; parts and resources are not.
        for k in [ContentKind::Actor, ContentKind::Item, ContentKind::Puzzle] {
            assert!(k.is_schedulable(), "{k} should be schedulable");
        }
        // ⚠ `UnlockTable` sits here and not above: the file is content, but it is *referenced* by
        // `grants` and `HoldsRule`, never placed. The atoms inside it are rows, which are data.
        for k in [
            ContentKind::UnlockTable,
            ContentKind::Component,
            ContentKind::Action,
            ContentKind::Shape,
            ContentKind::SurfaceProperty,
            ContentKind::StaticMesh,
            ContentKind::CurveTable,
        ] {
            assert!(!k.is_schedulable(), "{k} should not be schedulable");
        }

        let r = populated();
        let names: Vec<&str> = r.schedulable().map(|(_, e)| e.path()).collect();
        assert_eq!(
            names.len(),
            2,
            "door and key — not the component, the mesh, or the unlock table"
        );
    }

    #[test]
    fn iteration_is_deterministic_and_id_ordered() {
        let r = populated();
        let first: Vec<ObjectId> = r.iter().map(|(id, _)| id).collect();
        let second: Vec<ObjectId> = r.iter().map(|(id, _)| id).collect();
        assert_eq!(first, second);
        let mut sorted = first.clone();
        sorted.sort();
        assert_eq!(
            first, sorted,
            "iteration must be in id order, not insertion order"
        );
    }

    #[test]
    fn registry_round_trips() {
        let r = populated();
        let back: ContentRegistry = from_bytes(&to_bytes(&r)).unwrap();
        assert_eq!(back, r);
        assert_eq!(to_bytes(&back), to_bytes(&r));
    }

    #[test]
    fn of_kind_filters() {
        let r = populated();
        assert_eq!(r.of_kind(ContentKind::Actor).count(), 1);
        assert_eq!(r.of_kind(ContentKind::Spine).count(), 0);
    }
}
