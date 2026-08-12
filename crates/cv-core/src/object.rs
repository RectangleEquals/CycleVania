//! Object identity — the `Object` root that everything the algorithm creates carries.
//!
//! # Identity vs. location
//!
//! Two different things are deliberately kept apart:
//!
//! * A [`Handle`](crate::Handle) is a **location** — where a value sits in an arena *right now*. It is
//!   compact and fast, but it means nothing outside its arena and says nothing about what the thing is.
//! * An [`ObjectId`] is an **identity** — which object this *is*. It survives serialization, appears in
//!   the descriptor, in trace output, and in editor deep-links, and is what a human or a host refers to
//!   when they say "that door".
//!
//! Collapsing the two would be a mistake: handles must be reusable (slots recycle), while identity must
//! not be, or a reproduction bundle could resolve "the object the trace complained about" to a
//! different object entirely.
//!
//! # Two ways to get an id
//!
//! * [`IdAllocator`] hands out **sequential** ids. Deterministic because generation order is
//!   deterministic, but a given object's id shifts if anything earlier in the run changes.
//! * [`ObjectId::derived`] hashes a **namespace + path**, so identity is content-addressed: the id of
//!   `("actor", "crawler/door_heavy")` is the same in every run, every build, and every target, no
//!   matter what else the generator did. Registered L0 content (M05) wants this, because its identity
//!   should track *what it is*, not *when it was created*.

use cv_determinism::hash;
use std::fmt;

/// A stable, serializable object identity.
///
/// Displays as `#` followed by 16 lowercase hex digits (e.g. `#0a1b2c3d4e5f6071`), which is what shows
/// up in trace lines and editor deep-links.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(u64);

impl ObjectId {
    /// The reserved "no object" id. Never allocated and never derived.
    pub const NONE: ObjectId = ObjectId(0);

    /// Wrap a raw value. Prefer [`IdAllocator`] or [`ObjectId::derived`]; this exists for
    /// deserialization and for hosts that carry their own id space.
    pub const fn from_raw(raw: u64) -> Self {
        ObjectId(raw)
    }

    /// The underlying value.
    pub const fn to_raw(self) -> u64 {
        self.0
    }

    /// Is this the reserved [`ObjectId::NONE`]?
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }

    /// A **content-addressed** id derived from a namespace and a path.
    ///
    /// Stable across runs, builds and targets — the same `(namespace, path)` always yields the same
    /// id. Use it whenever identity should follow *what a thing is* rather than when it was made.
    pub fn derived(namespace: &str, path: &str) -> Self {
        let id = hash::combine(hash::fnv1a_str(namespace), hash::fnv1a_str(path));
        // Never collide with the reserved NONE.
        ObjectId(if id == 0 { 1 } else { id })
    }

    /// An id derived beneath an existing one — for sub-objects whose identity should follow their
    /// parent's (a component of an actor, a state of a puzzle).
    pub fn child(self, label: &str) -> Self {
        let id = hash::combine(self.0, hash::fnv1a_str(label));
        ObjectId(if id == 0 { 1 } else { id })
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:016x}", self.0)
    }
}

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_none() {
            write!(f, "ObjectId(NONE)")
        } else {
            write!(f, "ObjectId({self})")
        }
    }
}

/// Hands out sequential [`ObjectId`]s.
///
/// Deterministic given a deterministic generation order, which the pipeline guarantees. The counter is
/// serialized with the world so a resumed or round-tripped run never reissues an id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdAllocator {
    next: u64,
}

impl Default for IdAllocator {
    fn default() -> Self {
        IdAllocator::new()
    }
}

impl IdAllocator {
    /// A fresh allocator. The first id issued is `#…0001`.
    pub fn new() -> Self {
        IdAllocator { next: 1 }
    }

    /// Resume from a previously serialized counter.
    pub fn resuming_from(next: u64) -> Self {
        IdAllocator { next: next.max(1) }
    }

    /// The value the next [`IdAllocator::allocate`] will use — serialize this to resume.
    pub fn peek(&self) -> u64 {
        self.next
    }

    /// Issue the next id.
    ///
    /// # Panics
    /// On exhausting the 64-bit id space, which would otherwise wrap and duplicate identities.
    pub fn allocate(&mut self) -> ObjectId {
        let id = ObjectId(self.next);
        self.next = self.next.checked_add(1).expect("ObjectId space exhausted");
        id
    }
}

/// The fields every object carries — the Rust side of the design's `Object` root.
///
/// Deliberately minimal. Everything here is *universal*; anything that belongs to only some objects
/// (transforms, components, gates) lives on the concrete type, not in this header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectHeader {
    /// Stable identity.
    pub id: ObjectId,
    /// Human-facing name: shown in the editor, the trace, and diagnostics. Not an identity — two
    /// objects may share a name, and renaming does not change what an object *is*.
    pub name: String,
}

impl ObjectHeader {
    /// A header with an explicit id and name.
    pub fn new(id: ObjectId, name: impl Into<String>) -> Self {
        ObjectHeader {
            id,
            name: name.into(),
        }
    }

    /// A header with a content-addressed id derived from `namespace` and `name`.
    pub fn derived(namespace: &str, name: impl Into<String>) -> Self {
        let name = name.into();
        ObjectHeader {
            id: ObjectId::derived(namespace, &name),
            name,
        }
    }
}

/// Implemented by everything that lives in the object graph.
///
/// Kept tiny on purpose: it is the common denominator the trace, the editor inspector, and
/// serialization rely on, and every method a subclass might *override* belongs to the `api` surface
/// (M19) rather than here.
pub trait Object {
    /// This object's header.
    fn header(&self) -> &ObjectHeader;

    /// This object's header, mutably.
    fn header_mut(&mut self) -> &mut ObjectHeader;

    /// The type's name, as shown in the editor and diagnostics.
    fn type_name(&self) -> &'static str;

    /// Stable identity.
    fn id(&self) -> ObjectId {
        self.header().id
    }

    /// Human-facing name.
    fn name(&self) -> &str {
        &self.header().name
    }

    /// A one-line description for traces and errors, e.g. `Door "heavy_gate" #00000000000004d2`.
    fn describe(&self) -> String {
        format!("{} {:?} {}", self.type_name(), self.name(), self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Door {
        header: ObjectHeader,
    }
    impl Object for Door {
        fn header(&self) -> &ObjectHeader {
            &self.header
        }
        fn header_mut(&mut self) -> &mut ObjectHeader {
            &mut self.header
        }
        fn type_name(&self) -> &'static str {
            "Door"
        }
    }

    #[test]
    fn allocator_is_sequential_and_deterministic() {
        let mut a = IdAllocator::new();
        let ids: Vec<_> = (0..4).map(|_| a.allocate()).collect();
        assert_eq!(
            ids.iter().map(|i| i.to_raw()).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        // A fresh allocator repeats exactly.
        let mut b = IdAllocator::new();
        assert_eq!((0..4).map(|_| b.allocate()).collect::<Vec<_>>(), ids);
        // No id is ever NONE.
        assert!(ids.iter().all(|i| !i.is_none()));
    }

    #[test]
    fn allocator_resumes_without_reissuing() {
        let mut a = IdAllocator::new();
        a.allocate();
        a.allocate();
        let resumed = IdAllocator::resuming_from(a.peek());
        assert_eq!(resumed.peek(), 3);
        assert_eq!(
            IdAllocator::resuming_from(a.peek())
                .clone()
                .allocate()
                .to_raw(),
            3
        );
    }

    #[test]
    fn derived_ids_are_content_addressed() {
        // Same inputs → same id, with no shared allocator and no ordering dependency.
        assert_eq!(
            ObjectId::derived("actor", "crawler/door_heavy"),
            ObjectId::derived("actor", "crawler/door_heavy")
        );
        // Namespace and path both matter.
        assert_ne!(
            ObjectId::derived("actor", "door"),
            ObjectId::derived("item", "door")
        );
        assert_ne!(
            ObjectId::derived("actor", "door"),
            ObjectId::derived("actor", "lever")
        );
        // Never collides with the reserved NONE.
        assert!(!ObjectId::derived("", "").is_none());
    }

    #[test]
    fn child_ids_follow_their_parent() {
        let parent = ObjectId::derived("actor", "door");
        assert_eq!(parent.child("mesh"), parent.child("mesh"));
        assert_ne!(parent.child("mesh"), parent.child("collision"));
        // A different parent gives different children even for the same label.
        assert_ne!(
            parent.child("mesh"),
            ObjectId::derived("actor", "lever").child("mesh")
        );
    }

    #[test]
    fn ids_display_readably() {
        assert_eq!(ObjectId::from_raw(1234).to_string(), "#00000000000004d2");
        assert_eq!(format!("{:?}", ObjectId::NONE), "ObjectId(NONE)");
    }

    #[test]
    fn object_trait_describes_itself() {
        let mut door = Door {
            header: ObjectHeader::new(ObjectId::from_raw(1234), "heavy_gate"),
        };
        assert_eq!(door.id().to_raw(), 1234);
        assert_eq!(door.name(), "heavy_gate");
        assert_eq!(door.describe(), "Door \"heavy_gate\" #00000000000004d2");
        door.header_mut().name = "renamed".into();
        assert_eq!(door.name(), "renamed");
        // Renaming does not change identity.
        assert_eq!(door.id().to_raw(), 1234);
    }

    #[test]
    fn derived_header_matches_derived_id() {
        let h = ObjectHeader::derived("actor", "door");
        assert_eq!(h.id, ObjectId::derived("actor", "door"));
        assert_eq!(h.name, "door");
    }
}
