//! The **fingerprint** — an identity for a *recipe*, and the reproduction bundle built on it.
//!
//! # The one thing to understand: the seed is not in it
//!
//! Two values decide what a generated world looks like, and they are deliberately kept apart:
//!
//! * The **fingerprint** identifies the *recipe* — the core version, the registered content, the
//!   compiled scripts, the configuration. Everything about "what kind of worlds can this build make".
//! * The **seed** picks *which* world the recipe produces. It is a **runtime input**, chosen per world.
//!
//! So `same fingerprint + same seed ⇒ identical world`, and that is the entire reproducibility claim.
//! Putting the seed inside the fingerprint would collapse the distinction and make it useless: every
//! world would have its own "recipe", so you could never ask the actually-useful question — *"was this
//! world made by the same build as that one?"*
//!
//! Practically, that split is what lets a bug report be actionable. A player sends `fingerprint + seed`;
//! if your fingerprint matches theirs, you regenerate their exact world. If it does not, you know
//! immediately that the *build* differs and comparing worlds is meaningless.
//!
//! # How it is computed
//!
//! Inputs are written through the ordinary [`Writer`](crate::Writer) into a canonical byte sequence and
//! digested. Reusing the serializer is deliberate: it is already proven deterministic and
//! target-independent (no `usize`, fixed endianness, exact float bits), so the fingerprint inherits
//! those properties instead of re-deriving them. Configuration entries are sorted by key, so a caller
//! cannot change the fingerprint by supplying the same settings in a different order.

use crate::content::ContentRegistry;
use crate::serialize::{Deserialize, Reader, SerResult, Serialize, Writer};
use cv_determinism::hash;
use std::collections::BTreeMap;
use std::fmt;

/// Identity of a generation *recipe* — everything except the seed.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fingerprint(u64);

impl Fingerprint {
    /// Wrap a raw digest (deserialization, or a host carrying its own).
    pub const fn from_raw(raw: u64) -> Self {
        Fingerprint(raw)
    }

    /// The underlying digest.
    pub const fn to_raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({self})")
    }
}

/// A configuration value that participates in the fingerprint.
#[derive(Clone, Debug, PartialEq)]
enum ConfigValue {
    Str(String),
    U64(u64),
    I64(i64),
    F64(f64),
    Bool(bool),
}

impl ConfigValue {
    fn write(&self, w: &mut Writer) {
        match self {
            ConfigValue::Str(v) => {
                w.u8(0);
                w.str(v);
            }
            ConfigValue::U64(v) => {
                w.u8(1);
                w.u64(*v);
            }
            ConfigValue::I64(v) => {
                w.u8(2);
                w.i64(*v);
            }
            ConfigValue::F64(v) => {
                w.u8(3);
                w.f64(*v); // exact bits — 0.1 and 0.1000000000000001 are different recipes
            }
            ConfigValue::Bool(v) => {
                w.u8(4);
                w.bool(*v);
            }
        }
    }
}

/// Accumulates the inputs to a [`Fingerprint`].
///
/// Note what the API does *not* offer: any way to feed in a seed. That omission is the point — see the
/// module docs.
#[derive(Clone, Debug)]
pub struct FingerprintBuilder {
    core_version: String,
    /// Digest of the registered content, if any.
    content: Option<u64>,
    /// Compiled-script digests by id (M17). Ordered, so order of addition cannot matter.
    scripts: BTreeMap<String, u64>,
    /// Configuration, ordered by key for the same reason.
    config: BTreeMap<String, ConfigValue>,
}

impl FingerprintBuilder {
    /// Start from the core version — always an input, so a rebuilt engine never claims to reproduce a
    /// world it might generate differently.
    pub fn new(core_version: impl Into<String>) -> Self {
        FingerprintBuilder {
            core_version: core_version.into(),
            content: None,
            scripts: BTreeMap::new(),
            config: BTreeMap::new(),
        }
    }

    /// Start from *this* build's core version.
    pub fn for_this_build() -> Self {
        FingerprintBuilder::new(crate::version())
    }

    /// Fold in every registered piece of content: its id, kind, path, and source digest.
    pub fn content(mut self, registry: &ContentRegistry) -> Self {
        let mut w = Writer::new();
        w.write(registry); // BTreeMap-ordered, so canonical
        self.content = Some(hash::digest64(&w.finish()));
        self
    }

    /// Fold in a compiled script's digest (M17 supplies these from `.cvb` hashes).
    pub fn script(mut self, id: impl Into<String>, digest: u64) -> Self {
        self.scripts.insert(id.into(), digest);
        self
    }

    /// Fold in a string configuration value.
    pub fn config_str(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config
            .insert(key.into(), ConfigValue::Str(value.into()));
        self
    }

    /// Fold in an unsigned configuration value.
    pub fn config_u64(mut self, key: impl Into<String>, value: u64) -> Self {
        self.config.insert(key.into(), ConfigValue::U64(value));
        self
    }

    /// Fold in a signed configuration value.
    pub fn config_i64(mut self, key: impl Into<String>, value: i64) -> Self {
        self.config.insert(key.into(), ConfigValue::I64(value));
        self
    }

    /// Fold in a floating-point configuration value. Hashed by exact bits.
    pub fn config_f64(mut self, key: impl Into<String>, value: f64) -> Self {
        self.config.insert(key.into(), ConfigValue::F64(value));
        self
    }

    /// Fold in a boolean configuration value.
    pub fn config_bool(mut self, key: impl Into<String>, value: bool) -> Self {
        self.config.insert(key.into(), ConfigValue::Bool(value));
        self
    }

    /// Compute the fingerprint.
    pub fn finish(&self) -> Fingerprint {
        let mut w = Writer::new();
        // Field tags keep sections from running together: a config key that happened to look like a
        // script id could otherwise produce the same bytes as a different arrangement of inputs.
        w.u8(b'V');
        w.str(&self.core_version);
        w.u8(b'C');
        w.write(&self.content);
        w.u8(b'S');
        w.len(self.scripts.len());
        for (id, digest) in &self.scripts {
            w.str(id);
            w.u64(*digest);
        }
        w.u8(b'K');
        w.len(self.config.len());
        for (key, value) in &self.config {
            w.str(key);
            value.write(&mut w);
        }
        Fingerprint(hash::digest64(&w.finish()))
    }
}

/// Everything needed to reproduce a world, and to know whether reproduction is even meaningful.
///
/// This is what a bug report carries. [`ReproductionBundle::check`] answers the first question that
/// matters — *can this build reproduce it at all?* — before anyone wastes time comparing worlds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReproductionBundle {
    /// The recipe this world was made by.
    pub fingerprint: Fingerprint,
    /// The runtime input that selected this particular world.
    pub seed: u64,
    /// Digest of the world that was produced, when the bundle was made to verify against.
    pub output_digest: Option<u64>,
}

/// Why a bundle cannot be reproduced by the current build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReproductionError {
    /// The build differs: different core version, content, scripts, or config.
    FingerprintMismatch {
        expected: Fingerprint,
        actual: Fingerprint,
    },
    /// The build matches and the world was regenerated, but it came out different — a determinism bug.
    OutputMismatch { expected: u64, actual: u64 },
}

impl fmt::Display for ReproductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReproductionError::FingerprintMismatch { expected, actual } => write!(
                f,
                "this build cannot reproduce that world: recipe {actual} does not match {expected}. \
                 The core version, registered content, scripts, or configuration differ."
            ),
            ReproductionError::OutputMismatch { expected, actual } => write!(
                f,
                "same recipe and seed produced a different world ({actual:016x} vs {expected:016x}) — \
                 this is a determinism bug, not a configuration difference."
            ),
        }
    }
}

impl std::error::Error for ReproductionError {}

impl ReproductionBundle {
    /// A bundle for a world about to be, or just, generated.
    pub fn new(fingerprint: Fingerprint, seed: u64) -> Self {
        ReproductionBundle {
            fingerprint,
            seed,
            output_digest: None,
        }
    }

    /// Record the digest of the world produced, so a later regeneration can be verified.
    pub fn with_output(mut self, output_digest: u64) -> Self {
        self.output_digest = Some(output_digest);
        self
    }

    /// Digest a serializable world for [`ReproductionBundle::with_output`].
    pub fn digest_of<T: Serialize + ?Sized>(world: &T) -> u64 {
        let mut w = Writer::new();
        w.write(world);
        hash::digest64(&w.finish())
    }

    /// Can `current` reproduce this bundle?
    pub fn check(&self, current: Fingerprint) -> Result<(), ReproductionError> {
        if current != self.fingerprint {
            return Err(ReproductionError::FingerprintMismatch {
                expected: self.fingerprint,
                actual: current,
            });
        }
        Ok(())
    }

    /// Verify a regenerated world against the recorded output digest.
    ///
    /// Distinguishes the two failures that feel identical from the outside: a *different build*
    /// (fingerprint mismatch — expected, explainable) versus *the same build producing a different
    /// world* (output mismatch — a determinism bug).
    pub fn verify<T: Serialize + ?Sized>(
        &self,
        current: Fingerprint,
        regenerated: &T,
    ) -> Result<(), ReproductionError> {
        self.check(current)?;
        if let Some(expected) = self.output_digest {
            let actual = ReproductionBundle::digest_of(regenerated);
            if actual != expected {
                return Err(ReproductionError::OutputMismatch { expected, actual });
            }
        }
        Ok(())
    }
}

impl Serialize for Fingerprint {
    fn serialize(&self, w: &mut Writer) {
        w.u64(self.0);
    }
}

impl Deserialize for Fingerprint {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Fingerprint(r.u64()?))
    }
}

impl Serialize for ReproductionBundle {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.fingerprint);
        w.u64(self.seed);
        w.write(&self.output_digest);
    }
}

impl Deserialize for ReproductionBundle {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(ReproductionBundle {
            fingerprint: r.read()?,
            seed: r.u64()?,
            output_digest: r.read()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentKind;
    use crate::serialize::{from_bytes, to_bytes};

    fn registry() -> ContentRegistry {
        let mut r = ContentRegistry::new();
        r.register(ContentKind::Actor, "door", 0xAA).unwrap();
        r.register(ContentKind::Item, "key", 0xBB).unwrap();
        r
    }

    fn base() -> FingerprintBuilder {
        FingerprintBuilder::new("0.1.0")
            .content(&registry())
            .config_f64("worldScale", 1.0)
            .config_u64("reachCount", 6)
    }

    #[test]
    fn identical_inputs_give_identical_fingerprints() {
        assert_eq!(base().finish(), base().finish());
    }

    #[test]
    fn config_order_does_not_matter() {
        let a = FingerprintBuilder::new("0.1.0")
            .config_u64("a", 1)
            .config_u64("b", 2)
            .finish();
        let b = FingerprintBuilder::new("0.1.0")
            .config_u64("b", 2)
            .config_u64("a", 1)
            .finish();
        assert_eq!(
            a, b,
            "the same settings supplied in another order are the same recipe"
        );
    }

    #[test]
    fn changing_the_recipe_changes_the_fingerprint() {
        let original = base().finish();
        // A different core version.
        assert_ne!(original, base_with_version("0.2.0"));
        // A different config value...
        assert_ne!(original, base().config_u64("reachCount", 7).finish());
        // ...including a float that differs in the last bit.
        assert_ne!(
            original,
            base().config_f64("worldScale", 1.0 + f64::EPSILON).finish()
        );
        // An added config key.
        assert_ne!(original, base().config_bool("extra", false).finish());
        // A script digest.
        assert_ne!(original, base().script("door.cvs", 1).finish());
        assert_ne!(
            base().script("door.cvs", 1).finish(),
            base().script("door.cvs", 2).finish()
        );
    }

    fn base_with_version(v: &str) -> Fingerprint {
        FingerprintBuilder::new(v)
            .content(&registry())
            .config_f64("worldScale", 1.0)
            .config_u64("reachCount", 6)
            .finish()
    }

    #[test]
    fn changing_registered_content_changes_the_fingerprint() {
        let original = base().finish();

        // Adding content.
        let mut more = registry();
        more.register(ContentKind::Item, "dash", 0xCC).unwrap();
        assert_ne!(
            original,
            FingerprintBuilder::new("0.1.0")
                .content(&more)
                .config_f64("worldScale", 1.0)
                .config_u64("reachCount", 6)
                .finish()
        );

        // Same content, different *source* — behaviour changed even though the declaration did not.
        let mut altered = ContentRegistry::new();
        altered.register(ContentKind::Actor, "door", 0xAA).unwrap();
        altered.register(ContentKind::Item, "key", 0x99).unwrap(); // was 0xBB
        assert_ne!(
            original,
            FingerprintBuilder::new("0.1.0")
                .content(&altered)
                .config_f64("worldScale", 1.0)
                .config_u64("reachCount", 6)
                .finish()
        );
    }

    #[test]
    fn registration_order_does_not_change_the_fingerprint() {
        let mut forwards = ContentRegistry::new();
        forwards.register(ContentKind::Actor, "door", 0xAA).unwrap();
        forwards.register(ContentKind::Item, "key", 0xBB).unwrap();
        let mut backwards = ContentRegistry::new();
        backwards.register(ContentKind::Item, "key", 0xBB).unwrap();
        backwards
            .register(ContentKind::Actor, "door", 0xAA)
            .unwrap();
        assert_eq!(
            FingerprintBuilder::new("0.1.0").content(&forwards).finish(),
            FingerprintBuilder::new("0.1.0")
                .content(&backwards)
                .finish()
        );
    }

    #[test]
    fn the_seed_is_not_an_input() {
        // There is no builder method that takes a seed — the type system says so. What this test pins
        // is the consequence: two worlds from different seeds share one recipe.
        let recipe = base().finish();
        let world_a = ReproductionBundle::new(recipe, 1);
        let world_b = ReproductionBundle::new(recipe, 999_999);
        assert_eq!(world_a.fingerprint, world_b.fingerprint);
        assert_ne!(world_a.seed, world_b.seed);
        // Both are reproducible by this build.
        assert!(world_a.check(recipe).is_ok());
        assert!(world_b.check(recipe).is_ok());
    }

    #[test]
    fn a_bundle_from_another_build_is_rejected_with_the_reason() {
        let theirs = ReproductionBundle::new(base_with_version("0.2.0"), 42);
        let err = theirs.check(base().finish()).unwrap_err();
        assert!(matches!(err, ReproductionError::FingerprintMismatch { .. }));
        assert!(err.to_string().contains("cannot reproduce"));
    }

    #[test]
    fn verify_separates_a_different_build_from_a_determinism_bug() {
        let recipe = base().finish();
        let world = vec![1u64, 2, 3];
        let bundle =
            ReproductionBundle::new(recipe, 7).with_output(ReproductionBundle::digest_of(&world));

        // Regenerating the same world passes.
        assert!(bundle.verify(recipe, &world).is_ok());

        // A different world under the same recipe is a determinism bug, and says so.
        let drifted = vec![1u64, 2, 4];
        assert!(matches!(
            bundle.verify(recipe, &drifted),
            Err(ReproductionError::OutputMismatch { .. })
        ));
        assert!(bundle
            .verify(recipe, &drifted)
            .unwrap_err()
            .to_string()
            .contains("determinism bug"));

        // A different build fails earlier, and differently — you are not told to hunt a bug that
        // isn't there.
        assert!(matches!(
            bundle.verify(base_with_version("0.2.0"), &world),
            Err(ReproductionError::FingerprintMismatch { .. })
        ));
    }

    #[test]
    fn bundles_round_trip() {
        let b = ReproductionBundle::new(base().finish(), 0xDEAD_BEEF).with_output(0x1234);
        assert_eq!(from_bytes::<ReproductionBundle>(&to_bytes(&b)).unwrap(), b);
    }

    #[test]
    fn fingerprints_display_readably() {
        assert_eq!(
            Fingerprint::from_raw(0x1234).to_string(),
            "0000000000001234"
        );
    }
}
