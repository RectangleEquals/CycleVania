//! **Metadata** — the escape hatch that is not an escape from determinism.
//!
//! Every `Object` carries a key→value map a developer may write anything into. It exists because a
//! project always has one fact the core did not model, and the alternative to a metadata channel is a
//! project fork.
//!
//! # The `CV_` guard, and why it needs three layers
//!
//! ⚠ Keys beginning `CV_` are **core-reserved**. A project writing one would be overwriting the core's
//! own bookkeeping, and the failure would present as *"the generator started behaving strangely"* with
//! nothing pointing at the write.
//!
//! The guard is needed in three places because there are three ways a key arrives:
//!
//! | Layer | Catches |
//! |---|---|
//! | a **compile error** on a literal key | the common case, at the earliest possible moment |
//! | a **runtime rejection** on a computed key | `set_meta("CV_" + suffix)`, which no compiler sees |
//! | a rejection **at the binding boundary** | a host calling in from JavaScript, past both |
//!
//! Any one alone leaves a door open. This module implements the second and third — the first belongs
//! to the compiler, and the check here is what it will call.
//!
//! # Order
//!
//! ⚠ **Insertion-ordered in memory, key-sorted on serialize.** A developer reading the inspector sees
//! what they wrote in the order they wrote it; a fingerprint sees a canonical form. Picking one order
//! for both would either scramble the inspector or make two identical projects hash differently.

use crate::object::ObjectId;
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use cv_determinism::{Transform, Vec3};
use std::fmt;

/// The prefix the core reserves for itself.
pub const RESERVED_PREFIX: &str = "CV_";

/// What a metadata value may hold.
///
/// ⚠ **A closed set, deliberately.** An open `Any` would let content stash a handle to something the
/// fingerprint cannot see, and the first symptom would be a world that fails to reproduce with no
/// visible cause. Every form here is serialisable and hashable by construction.
#[derive(Clone, Debug, PartialEq)]
pub enum MetaValue {
    /// A yes or no.
    Bool(bool),
    /// A whole number.
    ///
    /// ⚠ **`i32`, not `i64`.** A JavaScript number is exact only below 2^53, and metadata crosses the
    /// binding seam constantly; a 64-bit integer would round silently on the way out.
    Int(i32),
    /// A real number.
    Float(f64),
    /// Text.
    Text(String),
    /// A position or direction.
    Vec3(Vec3),
    /// A placement.
    Transform(Transform),
    /// An ordered list.
    Array(Vec<MetaValue>),
    /// A named map.
    Map(Vec<(String, MetaValue)>),
    /// A reference to something with identity.
    Ref(ObjectId),
}

impl MetaValue {
    /// The form's name, for diagnostics.
    pub fn kind(&self) -> &'static str {
        match self {
            MetaValue::Bool(_) => "Bool",
            MetaValue::Int(_) => "Int",
            MetaValue::Float(_) => "Float",
            MetaValue::Text(_) => "String",
            MetaValue::Vec3(_) => "Vec3",
            MetaValue::Transform(_) => "Transform",
            MetaValue::Array(_) => "Array",
            MetaValue::Map(_) => "Map",
            MetaValue::Ref(_) => "Ref",
        }
    }

    /// Read it as a bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            MetaValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Read it as an integer.
    pub fn as_int(&self) -> Option<i32> {
        match self {
            MetaValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// Read it as a float.
    ///
    /// ⚠ An `Int` reads as a float too, because *"I wrote 3 and asked for a number"* is not a type
    /// error a developer would thank us for. The reverse does not hold: a float is not silently
    /// truncated to an int.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            MetaValue::Float(v) => Some(*v),
            MetaValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Read it as text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MetaValue::Text(v) => Some(v),
            _ => None,
        }
    }

    /// Read it as a reference.
    pub fn as_ref_id(&self) -> Option<ObjectId> {
        match self {
            MetaValue::Ref(v) => Some(*v),
            _ => None,
        }
    }
}

impl fmt::Display for MetaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetaValue::Bool(v) => write!(f, "{v}"),
            MetaValue::Int(v) => write!(f, "{v}"),
            MetaValue::Float(v) => write!(f, "{v}"),
            MetaValue::Text(v) => write!(f, "{v:?}"),
            MetaValue::Vec3(v) => write!(f, "({}, {}, {})", v.x, v.y, v.z),
            MetaValue::Transform(_) => write!(f, "<transform>"),
            MetaValue::Array(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            MetaValue::Map(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
            MetaValue::Ref(id) => write!(f, "#{id}"),
        }
    }
}

/// Why a metadata write was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetaError {
    /// The key is in the core's reserved namespace.
    Reserved { key: String },
    /// The key was empty.
    Empty,
}

impl fmt::Display for MetaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetaError::Reserved { key } => write!(
                f,
                "{key:?} is in the core's reserved `{RESERVED_PREFIX}` namespace"
            ),
            MetaError::Empty => f.write_str("a metadata key may not be empty"),
        }
    }
}

impl std::error::Error for MetaError {}

/// **The guard**, as one function the three layers all call.
///
/// ⚠ Having one implementation is the point: three checks written separately would eventually
/// disagree, and the one that was wrong would be whichever the attacker — or the accident — used.
pub fn check_key(key: &str) -> Result<(), MetaError> {
    if key.is_empty() {
        return Err(MetaError::Empty);
    }
    if key.starts_with(RESERVED_PREFIX) {
        return Err(MetaError::Reserved {
            key: key.to_string(),
        });
    }
    Ok(())
}

/// An object's metadata.
///
/// ⚠ **Insertion-ordered.** A `Vec` rather than a map, because the inspector shows what a developer
/// wrote in the order they wrote it, and the canonical form is produced at serialize time instead.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    entries: Vec<(String, MetaValue)>,
}

impl Metadata {
    /// Empty.
    pub fn new() -> Self {
        Metadata::default()
    }

    /// Write a key. **Refuses the reserved namespace.**
    pub fn set(&mut self, key: &str, value: MetaValue) -> Result<(), MetaError> {
        check_key(key)?;
        self.write_unchecked(key, value);
        Ok(())
    }

    /// Write a key from the core itself, reserved namespace included.
    ///
    /// ⚠ **Crate-internal.** This is the only path that may write `CV_`, and it is not public, so a
    /// project cannot reach it however it arrives — including through the bindings.
    ///
    /// The one caller is [`crate::handoff`], which stamps the six facts the design promises a host.
    pub(crate) fn set_core(&mut self, key: &str, value: MetaValue) {
        self.write_unchecked(key, value);
    }

    fn write_unchecked(&mut self, key: &str, value: MetaValue) {
        match self.entries.iter_mut().find(|(k, _)| k == key) {
            // ⚠ Overwriting in place rather than pushing, so a rewritten key keeps its original
            // position — a value changing must not make it jump to the bottom of the inspector.
            Some(slot) => slot.1 = value,
            None => self.entries.push((key.to_string(), value)),
        }
    }

    /// Read a key.
    pub fn get(&self, key: &str) -> Option<&MetaValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Is the key present?
    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Remove a key; `true` if it was there.
    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        self.entries.len() != before
    }

    /// Every key, **in insertion order**.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }

    /// Every entry, in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &MetaValue)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// How many keys.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Is it empty?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every entry, **key-sorted** — the canonical form.
    ///
    /// ⚠ What serialize and the fingerprint see. Two projects that wrote the same keys in a different
    /// order must hash the same, and only a canonical order makes that true.
    pub fn canonical(&self) -> Vec<(&str, &MetaValue)> {
        let mut out: Vec<(&str, &MetaValue)> = self.iter().collect();
        out.sort_by(|a, b| a.0.cmp(b.0));
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Wire form
// ---------------------------------------------------------------------------------------------

impl Serialize for MetaValue {
    fn serialize(&self, w: &mut Writer) {
        match self {
            MetaValue::Bool(v) => {
                w.u8(0);
                w.u8(u8::from(*v));
            }
            MetaValue::Int(v) => {
                w.u8(1);
                w.i32(*v);
            }
            MetaValue::Float(v) => {
                w.u8(2);
                w.f64(*v);
            }
            MetaValue::Text(v) => {
                w.u8(3);
                w.str(v);
            }
            MetaValue::Vec3(v) => {
                w.u8(4);
                w.f64(v.x);
                w.f64(v.y);
                w.f64(v.z);
            }
            MetaValue::Transform(t) => {
                w.u8(5);
                w.f64(t.translation.x);
                w.f64(t.translation.y);
                w.f64(t.translation.z);
                w.f64(t.rotation.x);
                w.f64(t.rotation.y);
                w.f64(t.rotation.z);
                w.f64(t.rotation.w);
                w.f64(t.scale.x);
                w.f64(t.scale.y);
                w.f64(t.scale.z);
            }
            MetaValue::Array(items) => {
                w.u8(6);
                w.write(items);
            }
            MetaValue::Map(entries) => {
                w.u8(7);
                w.u32(entries.len() as u32);
                for (k, v) in entries {
                    w.str(k);
                    w.write(v);
                }
            }
            MetaValue::Ref(id) => {
                w.u8(8);
                w.write(id);
            }
        }
    }
}

impl Deserialize for MetaValue {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(match r.u8()? {
            0 => MetaValue::Bool(r.u8()? != 0),
            1 => MetaValue::Int(r.i32()?),
            2 => MetaValue::Float(r.f64()?),
            3 => MetaValue::Text(r.str()?),
            4 => MetaValue::Vec3(Vec3::new(r.f64()?, r.f64()?, r.f64()?)),
            5 => {
                let translation = Vec3::new(r.f64()?, r.f64()?, r.f64()?);
                let rotation = cv_determinism::Quat::new(r.f64()?, r.f64()?, r.f64()?, r.f64()?);
                let scale = Vec3::new(r.f64()?, r.f64()?, r.f64()?);
                MetaValue::Transform(Transform {
                    translation,
                    rotation,
                    scale,
                })
            }
            6 => MetaValue::Array(r.read()?),
            7 => {
                let n = r.u32()?;
                let mut entries = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    entries.push((r.str()?, r.read()?));
                }
                MetaValue::Map(entries)
            }
            8 => MetaValue::Ref(r.read()?),
            _ => return Err(SerError::InvalidValue("unknown MetaValue tag")),
        })
    }
}

impl Serialize for Metadata {
    /// ⚠ **Key-sorted**, not insertion-ordered. Two projects that wrote the same keys in a different
    /// order must produce the same bytes, or the fingerprint would describe the typing order rather
    /// than the project.
    fn serialize(&self, w: &mut Writer) {
        let canonical = self.canonical();
        w.u32(canonical.len() as u32);
        for (k, v) in canonical {
            w.str(k);
            w.write(v);
        }
    }
}

impl Deserialize for Metadata {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let n = r.u32()?;
        let mut entries = Vec::with_capacity(n as usize);
        for _ in 0..n {
            entries.push((r.str()?, r.read()?));
        }
        Ok(Metadata { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::{from_bytes, to_bytes};

    // --- the guard --------------------------------------------------------------------------

    #[test]
    fn the_core_namespace_is_refused_however_the_key_was_built() {
        // ⚠ The runtime layer: `"CV_" + suffix` is a key no compiler sees, and it must still fail.
        let mut m = Metadata::new();
        let computed = format!("{RESERVED_PREFIX}{}", "rationale");
        assert!(matches!(
            m.set(&computed, MetaValue::Bool(true)),
            Err(MetaError::Reserved { .. })
        ));
        assert!(m.is_empty(), "and nothing was written");
    }

    #[test]
    fn a_refused_write_leaves_an_existing_value_alone() {
        // A rejection must not be a delete in disguise.
        let mut m = Metadata::new();
        m.set_core("CV_rationale", MetaValue::Text("core".into()));
        assert!(m
            .set("CV_rationale", MetaValue::Text("mine".into()))
            .is_err());
        assert_eq!(m.get("CV_rationale").unwrap().as_text(), Some("core"));
    }

    #[test]
    fn an_empty_key_is_refused_because_it_can_never_be_read_back() {
        let mut m = Metadata::new();
        assert_eq!(m.set("", MetaValue::Bool(true)), Err(MetaError::Empty));
    }

    #[test]
    fn a_key_that_merely_contains_the_prefix_is_fine() {
        // ⚠ The guard is on the *prefix*, not on the substring. `my_CV_note` is a developer's key and
        // refusing it would be the guard overreaching into names it has no claim on.
        let mut m = Metadata::new();
        assert!(m.set("my_CV_note", MetaValue::Bool(true)).is_ok());
        assert!(m.set("cv_lowercase", MetaValue::Bool(true)).is_ok());
    }

    #[test]
    fn the_guard_is_one_function_so_three_layers_cannot_disagree() {
        // ⚠ Three checks written separately would eventually diverge, and the one that was wrong would
        // be whichever route the accident took.
        assert!(check_key("CV_x").is_err());
        assert!(check_key("x").is_ok());
        assert!(check_key("").is_err());
    }

    // --- order ------------------------------------------------------------------------------

    #[test]
    fn memory_order_is_insertion_and_serialize_order_is_sorted() {
        // ⚠ The inspector shows what a developer wrote in the order they wrote it; the fingerprint
        // sees a canonical form. One order for both would either scramble the inspector or make two
        // identical projects hash differently.
        let mut m = Metadata::new();
        m.set("zebra", MetaValue::Int(1)).unwrap();
        m.set("apple", MetaValue::Int(2)).unwrap();
        m.set("mango", MetaValue::Int(3)).unwrap();

        assert_eq!(
            m.keys().collect::<Vec<_>>(),
            vec!["zebra", "apple", "mango"]
        );
        assert_eq!(
            m.canonical().iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            vec!["apple", "mango", "zebra"]
        );
    }

    #[test]
    fn two_projects_that_typed_the_same_keys_in_a_different_order_serialize_the_same() {
        let mut a = Metadata::new();
        a.set("x", MetaValue::Int(1)).unwrap();
        a.set("y", MetaValue::Int(2)).unwrap();

        let mut b = Metadata::new();
        b.set("y", MetaValue::Int(2)).unwrap();
        b.set("x", MetaValue::Int(1)).unwrap();

        assert_ne!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
        assert_eq!(to_bytes(&a), to_bytes(&b), "but the recipe is the same");
    }

    #[test]
    fn rewriting_a_key_keeps_its_place_rather_than_moving_it_to_the_end() {
        // A value changing must not make the row jump to the bottom of the inspector.
        let mut m = Metadata::new();
        m.set("a", MetaValue::Int(1)).unwrap();
        m.set("b", MetaValue::Int(2)).unwrap();
        m.set("a", MetaValue::Int(9)).unwrap();
        assert_eq!(m.keys().collect::<Vec<_>>(), vec!["a", "b"]);
        assert_eq!(m.get("a").unwrap().as_int(), Some(9));
        assert_eq!(m.len(), 2);
    }

    // --- the value set ----------------------------------------------------------------------

    #[test]
    fn an_int_reads_as_a_float_and_a_float_does_not_read_as_an_int() {
        // ⚠ *"I wrote 3 and asked for a number"* is not a type error a developer would thank us for.
        // Silent truncation in the other direction would be.
        assert_eq!(MetaValue::Int(3).as_float(), Some(3.0));
        assert_eq!(MetaValue::Float(3.7).as_int(), None);
    }

    #[test]
    fn the_integer_form_is_thirty_two_bit_because_it_crosses_the_binding_seam() {
        // ⚠ A JavaScript number is exact only below 2^53; a 64-bit integer would round silently on
        // the way out, and the corruption would look like data loss with no cause.
        let big = MetaValue::Int(i32::MAX);
        assert_eq!(
            from_bytes::<MetaValue>(&to_bytes(&big)).unwrap().as_int(),
            Some(i32::MAX)
        );
    }

    #[test]
    fn every_form_round_trips_on_the_wire() {
        // ⚠ The closed set is what makes this exhaustive. An open `Any` would let content stash
        // something the fingerprint cannot see, and the first symptom would be a world that fails to
        // reproduce with no visible cause.
        let values = vec![
            MetaValue::Bool(true),
            MetaValue::Int(-42),
            MetaValue::Float(1.5),
            MetaValue::Text("hello".into()),
            MetaValue::Vec3(Vec3::new(1.0, 2.0, 3.0)),
            MetaValue::Transform(Transform::IDENTITY),
            MetaValue::Array(vec![MetaValue::Int(1), MetaValue::Int(2)]),
            MetaValue::Map(vec![("k".into(), MetaValue::Bool(false))]),
            MetaValue::Ref(ObjectId::derived("actor", "thing")),
        ];
        for v in values {
            assert_eq!(from_bytes::<MetaValue>(&to_bytes(&v)).unwrap(), v, "{v:?}");
        }
    }

    #[test]
    fn a_nested_structure_survives_the_wire() {
        let v = MetaValue::Map(vec![(
            "outer".into(),
            MetaValue::Array(vec![
                MetaValue::Map(vec![("inner".into(), MetaValue::Int(7))]),
                MetaValue::Text("x".into()),
            ]),
        )]);
        assert_eq!(from_bytes::<MetaValue>(&to_bytes(&v)).unwrap(), v);
    }

    #[test]
    fn a_whole_metadata_map_round_trips() {
        let mut m = Metadata::new();
        m.set("b", MetaValue::Int(2)).unwrap();
        m.set("a", MetaValue::Text("one".into())).unwrap();
        let back: Metadata = from_bytes(&to_bytes(&m)).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.get("a").unwrap().as_text(), Some("one"));
    }

    // --- ordinary use -------------------------------------------------------------------------

    #[test]
    fn removing_reports_whether_there_was_anything_to_remove() {
        let mut m = Metadata::new();
        m.set("a", MetaValue::Bool(true)).unwrap();
        assert!(m.remove("a"));
        assert!(!m.remove("a"));
        assert!(!m.has("a"));
    }

    #[test]
    fn a_value_reads_back_for_a_trace_a_developer_understands() {
        assert_eq!(
            MetaValue::Vec3(Vec3::new(1.0, 2.0, 3.0)).to_string(),
            "(1, 2, 3)"
        );
        assert_eq!(
            MetaValue::Array(vec![MetaValue::Int(1), MetaValue::Bool(true)]).to_string(),
            "[1, true]"
        );
        assert_eq!(MetaValue::Int(3).kind(), "Int");
    }
}
