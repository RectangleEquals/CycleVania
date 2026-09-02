//! **The `CV_*` handoff** — what the core writes onto the output graph for the host.
//!
//! Metadata is an **output channel to the host, not a control channel for authoring**. It has two
//! halves and they are the reason the `CV_` prefix exists at all:
//!
//! | Half | Written by | Read by |
//! |---|---|---|
//! | free-form keys | the **developer**, during authoring | the host, after generation |
//! | **`CV_*` keys** | the **core**, at L5 | the host, after generation |
//!
//! ⚠ **The prefix is what makes the two separable when a host iterates.** A host walking every placed
//! instance wants to filter *"facts the generator derived"* from *"facts my designer typed"*, and
//! without a reserved namespace the only way to tell them apart is a hard-coded list of key names that
//! goes stale the first time anyone adds one.
//!
//! # Why this is metadata and not six more typed fields
//!
//! Some of these facts already exist as typed fields somewhere in the descriptor — a `Rationale` on an
//! instance, a `NodeKind` on a scope. ⚠ **Duplicating them here is the feature, not waste.** A host
//! walking one uniform map per object does not have to know which of five differently-shaped record
//! types holds the fact it wants; that is precisely the iteration the channel exists for.
//!
//! ⚠ **Authoring still never reads these.** The design's rule is that `core → authoring` goes through
//! typed accessors on the context, *never string comparison*. This channel points one way, at the host,
//! after the run is over.

use crate::descriptor::WorldDescriptor;
use crate::meta::{MetaValue, Metadata};
use crate::object::ObjectId;
use crate::placement::Role;

/// The core-reserved keys, in one place.
///
/// ⚠ **Named constants rather than string literals at each write site.** A typo in a stamped key
/// produces a host that silently finds nothing under the name it expected, and nothing else fails.
///
/// ⚠ **The key strings are `SCREAMING_SNAKE` too**, matching what they are: constants. A host reading
/// `CV_ROLE` beside its own `faction` can see at a glance which side wrote which, before it even
/// checks the prefix.
pub mod keys {
    /// What the thing turned out to be — `Role`, assigned *after* the search.
    pub const ROLE: &str = "CV_ROLE";
    /// Which pipeline layer committed it.
    pub const LAYER: &str = "CV_LAYER";
    /// Which accessibility sphere it fell in.
    pub const SPHERE: &str = "CV_SPHERE";
    /// The deterministic RNG path that produced it — how a host reproduces one decision.
    pub const SEED_PATH: &str = "CV_SEED_PATH";
    /// The unlocks obtaining it grants.
    pub const GRANTS: &str = "CV_GRANTS";
    /// Flags that describe the run rather than the object.
    pub const AMBIENT: &str = "CV_AMBIENT";

    /// Every reserved key the core writes.
    pub const ALL: [&str; 6] = [ROLE, LAYER, SPHERE, SEED_PATH, GRANTS, AMBIENT];
}

/// What the core knows about one placed thing at handoff time.
///
/// ⚠ **Every field is optional, and that is honest rather than lax.** A run that never ran the sphere
/// ladder has no sphere to report, and stamping `0` would tell a host *"sphere zero"* — a specific,
/// wrong claim. An absent key says *"the generator did not determine this"*, which is the truth.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoreFacts {
    /// What it turned out to be. ⚠ An **output** of the search, never an input to it.
    pub role: Option<Role>,
    /// The pipeline layer that committed it, `0..=5`.
    pub layer: Option<u8>,
    /// The accessibility sphere it fell in.
    pub sphere: Option<u32>,
    /// The RNG fork path that produced the decision.
    pub seed_path: Option<String>,
    /// The unlocks obtaining it grants.
    pub grants: Vec<ObjectId>,
    /// Run-level flags — *"projected"*, *"adopted"*, *"pushed out"*.
    pub ambient: Vec<String>,
}

impl CoreFacts {
    /// Nothing known yet.
    pub fn new() -> Self {
        CoreFacts::default()
    }

    /// What it turned out to be.
    pub fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    /// Which layer committed it.
    pub fn layer(mut self, layer: u8) -> Self {
        self.layer = Some(layer);
        self
    }

    /// Which sphere it fell in.
    pub fn sphere(mut self, sphere: u32) -> Self {
        self.sphere = Some(sphere);
        self
    }

    /// The RNG path behind the decision.
    pub fn seed_path(mut self, path: impl Into<String>) -> Self {
        self.seed_path = Some(path.into());
        self
    }

    /// An unlock this grants.
    pub fn grants(mut self, unlock: ObjectId) -> Self {
        self.grants.push(unlock);
        self
    }

    /// A run-level flag.
    pub fn ambient(mut self, flag: impl Into<String>) -> Self {
        self.ambient.push(flag.into());
        self
    }

    /// **Stamp these facts into a metadata map.**
    ///
    /// ⚠ Writes through the crate-internal reserved path, which is the *only* way `CV_` keys are
    /// written. Content cannot reach it however it arrives, including through the bindings.
    ///
    /// ⚠ **Absent facts write no key at all**, rather than a null or a zero. A host testing
    /// `has_meta("CV_SPHERE")` is asking *"did the generator determine this?"*, and a stamped null
    /// would answer yes.
    pub fn stamp(&self, meta: &mut Metadata) {
        if let Some(role) = self.role {
            meta.set_core(keys::ROLE, MetaValue::Text(role.to_string()));
        }
        if let Some(layer) = self.layer {
            meta.set_core(keys::LAYER, MetaValue::Int(i32::from(layer)));
        }
        if let Some(sphere) = self.sphere {
            meta.set_core(keys::SPHERE, MetaValue::Int(sphere as i32));
        }
        if let Some(path) = &self.seed_path {
            meta.set_core(keys::SEED_PATH, MetaValue::Text(path.clone()));
        }
        if !self.grants.is_empty() {
            meta.set_core(
                keys::GRANTS,
                MetaValue::Array(self.grants.iter().copied().map(MetaValue::Ref).collect()),
            );
        }
        if !self.ambient.is_empty() {
            meta.set_core(
                keys::AMBIENT,
                MetaValue::Array(self.ambient.iter().cloned().map(MetaValue::Text).collect()),
            );
        }
    }

    /// The facts as a fresh metadata map.
    pub fn to_metadata(&self) -> Metadata {
        let mut m = Metadata::new();
        self.stamp(&mut m);
        m
    }
}

/// Read the core's own keys back off a metadata map — **the host's side of the channel**.
///
/// ⚠ Provided so a host is not writing `"CV_ROLE"` as a string literal either. The same typo that
/// breaks a write breaks a read, and only one of the two is visible in a diff.
pub trait CoreMeta {
    /// The map to read.
    fn core_meta(&self) -> &Metadata;

    /// What the generator decided this turned out to be.
    fn core_role(&self) -> Option<&str> {
        self.core_meta()
            .get(keys::ROLE)
            .and_then(MetaValue::as_text)
    }

    /// Which pipeline layer committed it.
    fn core_layer(&self) -> Option<i32> {
        self.core_meta()
            .get(keys::LAYER)
            .and_then(MetaValue::as_int)
    }

    /// Which accessibility sphere it fell in.
    fn core_sphere(&self) -> Option<i32> {
        self.core_meta()
            .get(keys::SPHERE)
            .and_then(MetaValue::as_int)
    }

    /// The RNG path behind the decision.
    fn core_seed_path(&self) -> Option<&str> {
        self.core_meta()
            .get(keys::SEED_PATH)
            .and_then(MetaValue::as_text)
    }

    /// **Every key the developer wrote**, with the core's own excluded.
    ///
    /// ⚠ **The half of the channel this whole prefix exists for.** A host iterating placed content
    /// wants its designers' keys without hand-maintaining a list of the core's — and a hand-maintained
    /// list goes stale the first time the core adds one.
    fn authored_keys(&self) -> Vec<&str> {
        self.core_meta()
            .keys()
            .filter(|k| !k.starts_with(crate::meta::RESERVED_PREFIX))
            .collect()
    }

    /// Every key the **core** wrote.
    fn core_keys(&self) -> Vec<&str> {
        self.core_meta()
            .keys()
            .filter(|k| k.starts_with(crate::meta::RESERVED_PREFIX))
            .collect()
    }
}

impl CoreMeta for Metadata {
    fn core_meta(&self) -> &Metadata {
        self
    }
}

/// Stamp run-level facts onto the descriptor's own root.
///
/// ⚠ The seed and fingerprint are already typed fields on [`WorldDescriptor`]; they are stamped here
/// **as well** so a host walking metadata uniformly sees them without a special case for the root.
pub fn stamp_run(descriptor: &mut WorldDescriptor, ambient: &[String]) {
    let facts = CoreFacts::new()
        .seed_path(format!("seed:{}", descriptor.seed))
        .ambient(ambient.join(","));
    facts.stamp(&mut descriptor.meta);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::MetaError;

    fn unlock(n: &str) -> ObjectId {
        ObjectId::derived("unlock", n)
    }

    #[test]
    fn the_core_writes_the_six_keys_the_design_names() {
        // ⚠ The enumerated payload from `11-host.md` §7: role, layer, sphere, seed path, grants,
        // ambient flags. Any one missing is a fact a host was promised and does not get.
        let m = CoreFacts::new()
            .role(Role::Gate)
            .layer(2)
            .sphere(3)
            .seed_path("world/reach_1/area_0#place")
            .grants(unlock("dash"))
            .ambient("adopted")
            .to_metadata();

        for key in keys::ALL {
            assert!(m.has(key), "{key} was promised and not written");
        }
        // ⚠ The design's own spelling — `DECORATION`/`OBSTACLE`/`TRAVERSAL`/`GATE`.
        assert_eq!(m.core_role(), Some("GATE"));
        assert_eq!(m.core_layer(), Some(2));
        assert_eq!(m.core_sphere(), Some(3));
        assert_eq!(m.core_seed_path(), Some("world/reach_1/area_0#place"));
    }

    #[test]
    fn a_fact_the_generator_did_not_determine_writes_no_key() {
        // ⚠ Stamping `0` for an unrun sphere ladder would tell a host *"sphere zero"* — a specific,
        // wrong claim. Absence says *"not determined"*, which is the truth.
        let m = CoreFacts::new().role(Role::Decoration).to_metadata();
        assert!(m.has(keys::ROLE));
        assert!(!m.has(keys::SPHERE));
        assert_eq!(m.core_sphere(), None);
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn a_host_separates_its_designers_keys_from_the_generators() {
        // ⚠ **The half of the channel the prefix exists for.** Without it the only way to tell them
        // apart is a hard-coded list of key names that goes stale the first time anyone adds one.
        let mut m = Metadata::new();
        m.set("faction", MetaValue::Text("cult".into())).unwrap();
        m.set("vo_bank", MetaValue::Int(4)).unwrap();
        CoreFacts::new().role(Role::Obstacle).layer(4).stamp(&mut m);

        assert_eq!(m.authored_keys(), vec!["faction", "vo_bank"]);
        // Insertion order, as the memory form always is — `stamp` writes role before layer.
        assert_eq!(m.core_keys(), vec![keys::ROLE, keys::LAYER]);
        assert_eq!(m.len(), 4, "both halves live in one map");
    }

    #[test]
    fn content_still_cannot_write_a_reserved_key() {
        // The guard is unchanged by the core gaining a writer: the core's path is crate-internal, and
        // the public one refuses.
        let mut m = CoreFacts::new().role(Role::Gate).to_metadata();
        assert!(matches!(
            m.set(keys::ROLE, MetaValue::Text("decoration".into())),
            Err(MetaError::Reserved { .. })
        ));
        assert_eq!(m.core_role(), Some("GATE"), "and the core's value stands");
    }

    #[test]
    fn grants_are_a_list_because_one_pickup_may_open_several_gates() {
        // ⚠ The Speed Booster shape: one item, several separable unlocks. A scalar here would have
        // silently reported the first.
        let m = CoreFacts::new()
            .grants(unlock("dash"))
            .grants(unlock("shinespark"))
            .to_metadata();
        let MetaValue::Array(items) = m.get(keys::GRANTS).unwrap() else {
            panic!("an array")
        };
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn the_keys_are_constants_so_a_typo_cannot_be_written_on_one_side_only() {
        // ⚠ A typo in a stamped key produces a host that silently finds nothing under the name it
        // expected, and nothing else fails. The reader and the writer share the constant.
        assert_eq!(keys::ALL.len(), 6);
        for k in keys::ALL {
            assert!(k.starts_with(crate::meta::RESERVED_PREFIX), "{k}");
        }
    }

    #[test]
    fn stamping_twice_overwrites_rather_than_duplicating() {
        // A re-run of L5 must not leave two roles behind.
        let mut m = Metadata::new();
        CoreFacts::new().role(Role::Decoration).stamp(&mut m);
        CoreFacts::new().role(Role::Gate).stamp(&mut m);
        assert_eq!(m.len(), 1);
        assert_eq!(m.core_role(), Some("GATE"));
    }
}
