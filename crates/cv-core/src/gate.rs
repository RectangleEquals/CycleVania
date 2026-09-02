//! **Gate policy** — what qualifies a `gate()`, and what the proof is allowed to reason over.
//!
//! ⚠ **These live beside `gate()` because a gate is what they qualify.** Both are inert on an ungated
//! Actor: a rock with no `gate()` has nothing for a skip policy to police and nothing for a
//! discoverability estimate to be about. Attaching them anywhere else would invite a developer to set
//! them on content where they can never do anything.
//!
//! # Two things the core must be told rather than derive
//!
//! **`discoverability`.** *"The improbability of trying it unprompted"* — bombing a plain-looking wall,
//! standing still on a pressure plate for three seconds — **is not derivable from geometry.** The core
//! could pretend, and every pretence would be a guess presented as a fact. So a knowledge gate declares
//! it, and above the threshold the solver treats the gate as **open** (conservative, P1) **and names it
//! in the trace with its declared value** — so a developer sees which gate the solver assumed away.
//!
//! **`skip_policy`.** Whether alternative routes past *this* gate are tolerated, reported, or actively
//! forbidden. ⚠ **Per lock, and a designer marks two or three, not two hundred.** A global setting
//! would force one answer onto a shortcut that is a beloved speedrun trick and onto one that ruins the
//! act structure.
//!
//! # The quarantine, and why it is structural
//!
//! ⚠ **A quarantined variation never enters the proof.** A project marking a variation as quarantined —
//! a random affix, a cosmetic upgrade — means the solver evaluates **base capabilities only**, and
//! anything accessible *solely* through the variation is auto-tagged `BONUS`.
//!
//! Enforcement is the proof's **evaluation domain**, not a check someone remembers to run. That is the
//! only version that stays true as the solver grows: a rule *applied* at ten call sites is a rule
//! forgotten at the eleventh, whereas a set the proof is *built from* cannot be bypassed by a new
//! caller who never heard of it.

use crate::object::ObjectId;
use std::collections::BTreeSet;
use std::fmt;

/// What to do about alternative routes past a gate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SkipPolicy {
    /// A path exists; other emergent paths are fine.
    ///
    /// ⚠ **The default, because real games ship tolerated skips deliberately.** Defaulting to
    /// `Guarded` would make every unmarked gate pay for an absence proof, and a designer who wanted
    /// none of it would have to say so on every door.
    #[default]
    Tolerated,
    /// Report every alternative route found.
    ///
    /// ⚠ Reported, not refused — the developer decides. A solver that refused here would be making an
    /// act-structure judgement it has no standing to make.
    Exact,
    /// Actively verify no alternative exists at that sphere, and **fail loudly if one does**.
    Guarded,
}

impl SkipPolicy {
    /// Every policy, cheapest first.
    pub const ALL: [SkipPolicy; 3] = [
        SkipPolicy::Tolerated,
        SkipPolicy::Exact,
        SkipPolicy::Guarded,
    ];

    /// Does this policy require the solver to *search for* alternatives?
    ///
    /// ⚠ The cost question. `Tolerated` costs nothing; the other two pay for a search, which is why
    /// the default is the free one.
    pub fn requires_search(self) -> bool {
        !matches!(self, SkipPolicy::Tolerated)
    }

    /// Does finding an alternative fail the build?
    pub fn refuses_alternatives(self) -> bool {
        matches!(self, SkipPolicy::Guarded)
    }
}

impl fmt::Display for SkipPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SkipPolicy::Tolerated => "TOLERATED",
            SkipPolicy::Exact => "EXACT",
            SkipPolicy::Guarded => "GUARDED",
        })
    }
}

/// How likely a player is to try this unprompted, in `0..1`.
///
/// ⚠ **Declared, never derived**, and `1.0` by default — meaning *"obvious"*. A default of `0.0` would
/// silently make every ungated Actor into a hidden secret the solver routes around.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Discoverability(f64);

impl Discoverability {
    /// Obvious — the default.
    pub const OBVIOUS: Discoverability = Discoverability(1.0);

    /// A declared estimate, clamped to `0..1`.
    pub fn new(v: f64) -> Self {
        Discoverability(if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            1.0
        })
    }

    /// The declared value.
    pub fn value(self) -> f64 {
        self.0
    }

    /// Is this gate obscure enough that the solver must **not** assume a player finds it?
    ///
    /// ⚠ **Below the threshold the gate is real; above it the solver treats it as open** — the
    /// conservative direction (P1), because assuming a player finds a secret is how a world becomes
    /// completable only in theory.
    pub fn is_obscure(self, threshold: f64) -> bool {
        self.0 < threshold
    }
}

impl Default for Discoverability {
    fn default() -> Self {
        Discoverability::OBVIOUS
    }
}

impl fmt::Display for Discoverability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What a gate declares about itself.
///
/// ⚠ Both fields are **inert unless `gate()` is non-trivial**, which [`GatePolicy::applies`] answers —
/// so a developer who sets a skip policy on a rock gets told it does nothing, rather than believing it
/// does something.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GatePolicy {
    /// What to do about alternative routes past this gate.
    pub skip_policy: SkipPolicy,
    /// How likely a player is to try this unprompted.
    pub discoverability: Discoverability,
}

impl GatePolicy {
    /// The defaults: tolerated, obvious.
    pub fn new() -> Self {
        GatePolicy::default()
    }

    /// Set the skip policy.
    pub fn skipping(mut self, policy: SkipPolicy) -> Self {
        self.skip_policy = policy;
        self
    }

    /// Set the declared discoverability.
    pub fn discoverable(mut self, v: f64) -> Self {
        self.discoverability = Discoverability::new(v);
        self
    }

    /// Does any of this mean anything for an Actor whose gate is `gated`?
    ///
    /// ⚠ **Inert on an ungated Actor**, and that is worth reporting rather than ignoring: a policy set
    /// where it cannot act is a developer believing something is happening.
    pub fn applies(&self, gated: bool) -> bool {
        gated
    }

    /// Is this policy set to something other than the defaults?
    pub fn is_declared(&self) -> bool {
        self.skip_policy != SkipPolicy::Tolerated
            || self.discoverability != Discoverability::OBVIOUS
    }
}

/// **The proof's evaluation domain** — which unlocks the solver may reason over.
///
/// ⚠ **Quarantine is enforced here rather than at each call site.** A rule applied at ten call sites is
/// a rule forgotten at the eleventh; a *set the proof is built from* cannot be bypassed by a caller who
/// never heard of it. Every accessibility question routes through [`Domain::admits`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Domain {
    quarantined: BTreeSet<ObjectId>,
}

impl Domain {
    /// Everything counts.
    pub fn all() -> Self {
        Domain::default()
    }

    /// Quarantine a variation — a random affix, a cosmetic upgrade.
    pub fn quarantine(mut self, unlock: ObjectId) -> Self {
        self.quarantined.insert(unlock);
        self
    }

    /// Quarantine several.
    pub fn quarantining(mut self, unlocks: impl IntoIterator<Item = ObjectId>) -> Self {
        self.quarantined.extend(unlocks);
        self
    }

    /// May the proof reason over this unlock?
    pub fn admits(&self, unlock: ObjectId) -> bool {
        !self.quarantined.contains(&unlock)
    }

    /// Is this unlock quarantined?
    pub fn is_quarantined(&self, unlock: ObjectId) -> bool {
        self.quarantined.contains(&unlock)
    }

    /// How many variations are quarantined.
    pub fn len(&self) -> usize {
        self.quarantined.len()
    }

    /// Is nothing quarantined?
    pub fn is_empty(&self) -> bool {
        self.quarantined.is_empty()
    }

    /// **The base capabilities** — a held set with every quarantined variation removed.
    ///
    /// ⚠ This is what the proof runs on. Filtering at the *domain* rather than at the caller is what
    /// makes *"a quarantined variation never enters the proof"* structural.
    pub fn base(&self, held: &BTreeSet<ObjectId>) -> BTreeSet<ObjectId> {
        held.iter().copied().filter(|u| self.admits(*u)).collect()
    }

    /// Was this accessible **only** through a quarantined variation?
    ///
    /// ⚠ The auto-`BONUS` test. Something the base capabilities cannot reach but the full set can was
    /// opened by a variation, and a variation may not gate progression.
    pub fn only_via_quarantine(&self, with_base: bool, with_all: bool) -> bool {
        with_all && !with_base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unlock(n: &str) -> ObjectId {
        ObjectId::derived("unlock", n)
    }

    // --- skip policy ---------------------------------------------------------------------------

    #[test]
    fn tolerated_is_the_default_because_real_games_ship_skips_deliberately() {
        // ⚠ Defaulting to `Guarded` would make every unmarked gate pay for an absence proof, and a
        // designer who wanted none of it would have to say so on every door.
        assert_eq!(SkipPolicy::default(), SkipPolicy::Tolerated);
        assert!(!SkipPolicy::default().requires_search(), "and it is free");
    }

    #[test]
    fn only_guarded_refuses_an_alternative() {
        // ⚠ `Exact` *reports*; refusing there would be the solver making an act-structure judgement it
        // has no standing to make.
        assert!(!SkipPolicy::Tolerated.refuses_alternatives());
        assert!(!SkipPolicy::Exact.refuses_alternatives());
        assert!(SkipPolicy::Guarded.refuses_alternatives());
        assert!(SkipPolicy::Exact.requires_search(), "but it still searches");
    }

    #[test]
    fn the_policies_render_as_the_design_names_them() {
        let names: Vec<String> = SkipPolicy::ALL.iter().map(|p| p.to_string()).collect();
        assert_eq!(names, vec!["TOLERATED", "EXACT", "GUARDED"]);
    }

    // --- discoverability -----------------------------------------------------------------------

    #[test]
    fn obvious_is_the_default_because_the_other_way_hides_everything() {
        // ⚠ A default of `0.0` would silently make every ungated Actor a hidden secret the solver
        // routes around — a world that generates and is unplayable in a way nothing reports.
        assert_eq!(Discoverability::default(), Discoverability::OBVIOUS);
        assert_eq!(Discoverability::default().value(), 1.0);
    }

    #[test]
    fn above_the_threshold_a_gate_is_treated_as_open() {
        // ⚠ The conservative direction (P1): assuming a player finds a secret is how a world becomes
        // completable only in theory.
        let obscure = Discoverability::new(0.05); // bombing a plain-looking wall
        let plain = Discoverability::new(0.9);
        assert!(
            obscure.is_obscure(0.5),
            "the solver must not assume this one"
        );
        assert!(!plain.is_obscure(0.5), "this one it may treat as open");
    }

    #[test]
    fn a_nonsense_estimate_degrades_to_obvious_rather_than_to_hidden() {
        // NaN or infinity must not turn a door into a secret.
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(Discoverability::new(bad).value(), 1.0, "{bad}");
        }
        assert_eq!(Discoverability::new(-3.0).value(), 0.0);
        assert_eq!(Discoverability::new(7.0).value(), 1.0);
    }

    // --- both are inert on an ungated Actor ------------------------------------------------------

    #[test]
    fn a_policy_on_an_ungated_actor_does_nothing_and_says_so() {
        // ⚠ A policy set where it cannot act is a developer believing something is happening. Better
        // to be able to ask than to leave them guessing.
        let p = GatePolicy::new()
            .skipping(SkipPolicy::Guarded)
            .discoverable(0.1);
        assert!(p.is_declared());
        assert!(p.applies(true), "on a gated Actor it means something");
        assert!(!p.applies(false), "on a rock it does not");
    }

    #[test]
    fn the_defaults_are_not_a_declaration() {
        assert!(!GatePolicy::new().is_declared());
        assert!(GatePolicy::new().skipping(SkipPolicy::Exact).is_declared());
    }

    // --- the quarantine ------------------------------------------------------------------------

    #[test]
    fn a_quarantined_variation_never_enters_the_proof() {
        // ⚠ **Structural**: the proof runs on `base`, so a caller who never heard of quarantine cannot
        // bypass it. A rule applied at ten call sites is forgotten at the eleventh.
        let affix = unlock("fire_affix");
        let dash = unlock("dash");
        let domain = Domain::all().quarantine(affix);

        let held: BTreeSet<ObjectId> = [affix, dash].into_iter().collect();
        let base = domain.base(&held);

        assert!(base.contains(&dash));
        assert!(!base.contains(&affix), "the variation is not in the domain");
        assert!(!domain.admits(affix));
        assert!(domain.admits(dash));
    }

    #[test]
    fn something_accessible_only_through_a_variation_is_flagged() {
        // ⚠ The auto-`BONUS` test. A variation may not gate progression, so anything it alone opens is
        // optional by construction rather than by someone remembering to tag it.
        let domain = Domain::all().quarantine(unlock("fire_affix"));
        assert!(
            domain.only_via_quarantine(false, true),
            "inaccessible on base, accessible with the affix"
        );
        assert!(
            !domain.only_via_quarantine(true, true),
            "accessible either way"
        );
        assert!(
            !domain.only_via_quarantine(false, false),
            "inaccessible either way"
        );
    }

    #[test]
    fn an_empty_quarantine_admits_everything() {
        let domain = Domain::all();
        assert!(domain.is_empty());
        let held: BTreeSet<ObjectId> = [unlock("a"), unlock("b")].into_iter().collect();
        assert_eq!(domain.base(&held), held);
    }

    #[test]
    fn quarantining_several_at_once_is_the_same_as_one_at_a_time() {
        let a = Domain::all().quarantining([unlock("x"), unlock("y")]);
        let b = Domain::all()
            .quarantine(unlock("x"))
            .quarantine(unlock("y"));
        assert_eq!(a, b);
        assert_eq!(a.len(), 2);
    }
}
