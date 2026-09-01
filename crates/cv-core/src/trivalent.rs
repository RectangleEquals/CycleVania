//! **Three-valued answers, and the bounded error that makes them honest.**
//!
//! # Why a bool is the wrong return type here
//!
//! > **A confident wrong answer is worse than an admitted unknown.**
//!
//! At L2 the geometry is approximate. A query asked *"is that ledge within 30 metres?"* against a convex
//! hull is being asked about a shape that does not exist yet — the real one is somewhere inside it. A
//! `bool` forces that query to pick a side, and whichever side it picks it will sometimes be wrong with
//! no way for the caller to know which times.
//!
//! [`Trivalent`] lets it decline. `AMBIGUOUS` is not a failure and not a maybe-yes: it means *the
//! question cannot be answered at this fidelity*, and the decision re-asks at the next rung.
//!
//! # The ladder, and why the error only shrinks
//!
//! ```text
//! ENVELOPE  ⊇  HULL  ⊇  GEOMETRY
//!   slack      contouring    0
//! ```
//!
//! [`crate::floor`] establishes that each rung only ever **tightens**. Tolerance is the bounded error of
//! a rung, so it only ever **shrinks** — and that is where the guarantee comes from:
//!
//! > **A decision made outside the ambiguous band at L2 cannot be overturned at L4.**
//!
//! It is a property of the *ordering*, not of extra machinery. If `measured − ε` already exceeds the
//! limit, no later refinement can bring it back under, because later refinements can only move the
//! measurement by less than ε. That claim is what `no_l2_verdict_is_ever_contradicted_at_l4` exists to
//! test across a thousand seeds, and it is the milestone's real deliverable — checking the three
//! branches would not have tested it at all.
//!
//! # No `PENDING_GEOMETRY`
//!
//! ⚠ A decision that returns `AMBIGUOUS` re-asks at the next rung **by construction**, so there is
//! deliberately no marker value for *"come back later"*. A marker would have to be stored, propagated
//! and cleared, and every one of those is a chance to leave one behind.

use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use cv_determinism::math;
use std::fmt;

/// Three-valued truth.
///
/// Returned wherever geometry is still approximate, which is everywhere below `GEOMETRY` fidelity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Trivalent {
    /// Definitely true, and no later rung can overturn it.
    Yes,
    /// Definitely false, and no later rung can overturn it.
    No,
    /// Inside the ambiguous band.
    ///
    /// ⚠ **Resolve, never guess.** A decision returning this re-asks at the next fidelity rung.
    Ambiguous,
}

impl Trivalent {
    /// A definite answer from a source that cannot be wrong.
    ///
    /// ⚠ Use only where the question is genuinely exact — set membership in a committed volume, an
    /// id comparison. Wrapping an approximate measurement with this is how a bool sneaks back in.
    pub fn exact(b: bool) -> Self {
        if b {
            Trivalent::Yes
        } else {
            Trivalent::No
        }
    }

    /// Is this a definite answer either way?
    pub fn is_definite(self) -> bool {
        self != Trivalent::Ambiguous
    }

    /// The answer if it is definite, `None` if it is not.
    ///
    /// ⚠ Deliberately not `unwrap_or(false)`: collapsing `AMBIGUOUS` to `false` at a call site is
    /// exactly the confident wrong answer the type exists to prevent.
    pub fn definite(self) -> Option<bool> {
        match self {
            Trivalent::Yes => Some(true),
            Trivalent::No => Some(false),
            Trivalent::Ambiguous => None,
        }
    }

    /// Both must hold. Ambiguity is contagious unless a definite `NO` settles it.
    pub fn and(self, other: Trivalent) -> Trivalent {
        match (self, other) {
            (Trivalent::No, _) | (_, Trivalent::No) => Trivalent::No,
            (Trivalent::Yes, Trivalent::Yes) => Trivalent::Yes,
            _ => Trivalent::Ambiguous,
        }
    }

    /// Either may hold. A definite `YES` settles it.
    pub fn or(self, other: Trivalent) -> Trivalent {
        match (self, other) {
            (Trivalent::Yes, _) | (_, Trivalent::Yes) => Trivalent::Yes,
            (Trivalent::No, Trivalent::No) => Trivalent::No,
            _ => Trivalent::Ambiguous,
        }
    }

    /// Swaps the definite answers; ambiguity negates to ambiguity.
    pub fn negate(self) -> Trivalent {
        match self {
            Trivalent::Yes => Trivalent::No,
            Trivalent::No => Trivalent::Yes,
            Trivalent::Ambiguous => Trivalent::Ambiguous,
        }
    }
}

impl fmt::Display for Trivalent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Trivalent::Yes => "yes",
            Trivalent::No => "no",
            Trivalent::Ambiguous => "ambiguous",
        })
    }
}

impl Trivalent {
    /// The wire tag. Explicit rather than derived from declaration order, so reordering the enum
    /// cannot silently change a reproduction bundle.
    fn tag(self) -> u8 {
        match self {
            Trivalent::Yes => 0,
            Trivalent::No => 1,
            Trivalent::Ambiguous => 2,
        }
    }

    fn from_tag(t: u8) -> Option<Self> {
        match t {
            0 => Some(Trivalent::Yes),
            1 => Some(Trivalent::No),
            2 => Some(Trivalent::Ambiguous),
            _ => None,
        }
    }
}

impl Serialize for Trivalent {
    fn serialize(&self, w: &mut Writer) {
        w.u8(self.tag());
    }
}

impl Deserialize for Trivalent {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Trivalent::from_tag(r.u8()?).ok_or(SerError::InvalidValue("unknown Trivalent tag"))
    }
}

/// How real the geometry currently is.
///
/// ⚠ **Fidelity is what *exists*; detail is what a query *asks for*.** Conflating them is how a query
/// ends up believing it got an answer the world could not yet supply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fidelity {
    /// Envelopes only. Tolerance is the envelope's own slack.
    Envelope,
    /// Hulls exist. Tolerance is the contouring tolerance.
    Hull,
    /// Real geometry. Tolerance is zero.
    Geometry,
}

impl Fidelity {
    /// Outermost first — the order the ladder climbs.
    pub const ALL: [Fidelity; 3] = [Fidelity::Envelope, Fidelity::Hull, Fidelity::Geometry];

    /// The next rung, or `None` at the bottom.
    ///
    /// This is the deferral path: an `AMBIGUOUS` answer re-asks here.
    pub fn next(self) -> Option<Fidelity> {
        match self {
            Fidelity::Envelope => Some(Fidelity::Hull),
            Fidelity::Hull => Some(Fidelity::Geometry),
            Fidelity::Geometry => None,
        }
    }
}

impl Fidelity {
    fn tag(self) -> u8 {
        match self {
            Fidelity::Envelope => 0,
            Fidelity::Hull => 1,
            Fidelity::Geometry => 2,
        }
    }

    fn from_tag(t: u8) -> Option<Self> {
        match t {
            0 => Some(Fidelity::Envelope),
            1 => Some(Fidelity::Hull),
            2 => Some(Fidelity::Geometry),
            _ => None,
        }
    }
}

impl Serialize for Fidelity {
    fn serialize(&self, w: &mut Writer) {
        w.u8(self.tag());
    }
}

impl Deserialize for Fidelity {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Fidelity::from_tag(r.u8()?).ok_or(SerError::InvalidValue("unknown Fidelity tag"))
    }
}

/// The bounded error at each rung, and the floor beneath all of them.
///
/// ▶ **The values themselves were a live design gap**: the design names a tolerance per rung and never
/// quantifies it. These are the defaults this milestone commits to, expressed relative to `world_scale`
/// so they mean the same thing in a project that measures in centimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerances {
    /// Slack in an envelope, in world units.
    ///
    /// An AABB over a room is loose by roughly the room's own irregularity; a half-metre at human
    /// scale is the honest order of magnitude, not a tuned number.
    pub envelope: f64,
    /// Contouring tolerance for a hull, in world units.
    pub hull: f64,
    /// ⚠ **The project-wide floor beneath a rung's own tolerance.**
    ///
    /// Even at `GEOMETRY`, where the rung's tolerance is zero, two measurements closer together than
    /// this are not meaningfully different — floating point saw to that. Without a floor, exact
    /// fidelity would answer `YES`/`NO` on differences that are numerical noise.
    pub ambiguity_epsilon: f64,
}

impl Tolerances {
    /// Defaults for a project at the given scale, with the project's own ambiguity floor.
    ///
    /// ⚠ **`ambiguity_epsilon` comes from [`crate::settings::Settings`], not from here.** It is a
    /// project-wide declaration — a dev who says *"a quarter of a unit is noise in my game"* must have
    /// that respected — and a default invented locally would be a setting nobody reads.
    pub fn new(world_scale: f64, ambiguity_epsilon: f64) -> Self {
        let u = if world_scale > 0.0 { world_scale } else { 1.0 };
        Tolerances {
            envelope: 0.5 * u,
            hull: 0.05 * u,
            ambiguity_epsilon: math::max(ambiguity_epsilon, 0.0),
        }
    }

    /// Read straight from a project's settings — the only path a real generation pass uses.
    pub fn from_settings(settings: &crate::settings::Settings) -> Self {
        Tolerances::new(settings.world_scale, settings.ambiguity_epsilon)
    }

    /// Defaults for a project at the given scale, with a nominal ambiguity floor.
    ///
    /// For tests and detached contexts. ⚠ Real passes use [`Tolerances::from_settings`].
    pub fn for_scale(world_scale: f64) -> Self {
        let u = if world_scale > 0.0 { world_scale } else { 1.0 };
        Tolerances::new(u, 1e-4 * u)
    }

    /// The bounded error at a rung, never below [`Tolerances::ambiguity_epsilon`].
    pub fn at(&self, fidelity: Fidelity) -> f64 {
        let rung = match fidelity {
            Fidelity::Envelope => self.envelope,
            Fidelity::Hull => self.hull,
            Fidelity::Geometry => 0.0,
        };
        math::max(rung, self.ambiguity_epsilon)
    }
}

impl Default for Tolerances {
    fn default() -> Self {
        Tolerances::for_scale(1.0)
    }
}

/// Is `measured` within `limit`, allowing for the rung's bounded error?
///
/// ```text
/// measured + ε  <  limit   ⇒  YES
/// measured − ε  >  limit   ⇒  NO
/// otherwise                ⇒  AMBIGUOUS
/// ```
///
/// ⚠ **The band is what makes the answer safe to act on.** A `YES` here survives every later rung,
/// because later rungs move the measurement by less than ε — which is the whole reason the ladder has
/// to be monotone before this function is worth anything.
pub fn within(measured: f64, limit: f64, tolerance: f64) -> Trivalent {
    let eps = math::abs(tolerance);
    if measured + eps < limit {
        Trivalent::Yes
    } else if measured - eps > limit {
        Trivalent::No
    } else {
        Trivalent::Ambiguous
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cv_determinism::Rng;

    #[test]
    fn the_wire_form_round_trips_and_is_order_independent() {
        use crate::serialize::{from_bytes, to_bytes};
        for v in [Trivalent::Yes, Trivalent::No, Trivalent::Ambiguous] {
            assert_eq!(from_bytes::<Trivalent>(&to_bytes(&v)).unwrap(), v);
        }
        for f in Fidelity::ALL {
            assert_eq!(from_bytes::<Fidelity>(&to_bytes(&f)).unwrap(), f);
        }
        // ⚠ Tags are explicit, so reordering the enum cannot silently rewrite a bundle. The payload
        // is the final byte; everything before it is the container's own header.
        assert_eq!(*to_bytes(&Trivalent::Ambiguous).last().unwrap(), 2u8);
        assert_eq!(*to_bytes(&Trivalent::Yes).last().unwrap(), 0u8);
        assert_eq!(*to_bytes(&Fidelity::Geometry).last().unwrap(), 2u8);
    }

    #[test]
    fn the_three_branches_are_the_three_branches() {
        let t = 1.0;
        assert_eq!(within(5.0, 10.0, t), Trivalent::Yes);
        assert_eq!(within(15.0, 10.0, t), Trivalent::No);
        assert_eq!(within(10.2, 10.0, t), Trivalent::Ambiguous);
    }

    #[test]
    fn the_band_is_symmetric_around_the_limit() {
        let t = 2.0;
        assert_eq!(within(8.1, 10.0, t), Trivalent::Ambiguous, "just inside");
        assert_eq!(within(11.9, 10.0, t), Trivalent::Ambiguous, "just outside");
        assert_eq!(within(7.9, 10.0, t), Trivalent::Yes);
        assert_eq!(within(12.1, 10.0, t), Trivalent::No);
    }

    #[test]
    fn zero_tolerance_still_leaves_the_boundary_ambiguous() {
        // ⚠ Exactly at the limit is not "within" and not "beyond". Answering either way would be a
        // coin flip dressed as a fact.
        assert_eq!(within(10.0, 10.0, 0.0), Trivalent::Ambiguous);
    }

    #[test]
    fn tolerance_only_shrinks_as_the_ladder_climbs() {
        let t = Tolerances::default();
        let e = t.at(Fidelity::Envelope);
        let h = t.at(Fidelity::Hull);
        let g = t.at(Fidelity::Geometry);
        assert!(e > h && h > g, "{e} > {h} > {g}");
    }

    #[test]
    fn tolerance_never_falls_below_the_project_floor() {
        // ⚠ At GEOMETRY the rung's own tolerance is zero, but two measurements closer than the
        // epsilon are still not meaningfully different.
        let t = Tolerances::default();
        assert_eq!(t.at(Fidelity::Geometry), t.ambiguity_epsilon);
        assert!(t.at(Fidelity::Geometry) > 0.0);
    }

    #[test]
    fn the_ambiguity_floor_comes_from_project_settings() {
        // ⚠ The setting existed before this milestone and **nothing read it**. A project-wide
        // declaration that no code consumes is not a setting; it is a comment with a type.
        let settings = crate::settings::Settings {
            ambiguity_epsilon: 2.5,
            ..Default::default()
        };
        let t = Tolerances::from_settings(&settings);
        assert_eq!(t.ambiguity_epsilon, 2.5);
        assert_eq!(
            t.at(Fidelity::Geometry),
            2.5,
            "the project's floor applies even where the rung's own tolerance is zero"
        );
        assert_eq!(
            t.at(Fidelity::Hull),
            2.5,
            "and it raises a rung whose own tolerance is finer than the project calls meaningful"
        );
    }

    #[test]
    fn tolerances_scale_with_the_project() {
        // A project measuring in centimetres must get the same *physical* slack.
        let m = Tolerances::for_scale(1.0);
        let cm = Tolerances::for_scale(100.0);
        assert_eq!(cm.envelope, m.envelope * 100.0);
        assert_eq!(cm.ambiguity_epsilon, m.ambiguity_epsilon * 100.0);
    }

    #[test]
    fn deferral_walks_the_ladder_and_stops() {
        assert_eq!(Fidelity::Envelope.next(), Some(Fidelity::Hull));
        assert_eq!(Fidelity::Hull.next(), Some(Fidelity::Geometry));
        assert_eq!(
            Fidelity::Geometry.next(),
            None,
            "there is nowhere to defer to from exact geometry"
        );
    }

    #[test]
    fn ambiguity_is_contagious_but_a_definite_no_settles_a_conjunction() {
        use Trivalent::*;
        assert_eq!(Yes.and(Ambiguous), Ambiguous);
        assert_eq!(No.and(Ambiguous), No, "one definite failure is enough");
        assert_eq!(Ambiguous.or(Yes), Yes, "one definite success is enough");
        assert_eq!(Ambiguous.or(No), Ambiguous);
        assert_eq!(Ambiguous.negate(), Ambiguous);
    }

    #[test]
    fn an_ambiguous_answer_cannot_be_read_as_false() {
        // ⚠ `definite()` returns None rather than collapsing — the API must not make "I don't know"
        // cheap to mistake for "no".
        assert_eq!(Trivalent::Ambiguous.definite(), None);
        assert_eq!(Trivalent::No.definite(), Some(false));
    }

    // -----------------------------------------------------------------------------------------
    // The milestone's real deliverable
    // -----------------------------------------------------------------------------------------

    #[test]
    fn no_l2_verdict_is_ever_contradicted_at_l4() {
        // ⚠ **This is what the tolerance machinery exists for.** Checking the three branches proves
        // the function has three branches. It does not prove the *claim*, which is:
        //
        //   > a decision made outside the ambiguous band at L2 cannot be overturned at L4
        //
        // The setup: a true measurement, and an L2 estimate that is wrong by up to the rung's own
        // tolerance — the worst the ladder permits. If a definite L2 answer ever disagrees with the
        // exact L4 answer, the band is too narrow and the guarantee is a fiction.
        let tol = Tolerances::default();
        let rng = Rng::new(0xB0_1D);
        let mut definite_at_l2 = 0u32;

        for i in 0..1_000u64 {
            let s = rng.fork_index(i);
            let limit = 1.0 + s.fork("limit").below(4_000) as f64 / 100.0;
            let truth = s.fork("truth").below(6_000) as f64 / 100.0;

            for rung in [Fidelity::Envelope, Fidelity::Hull] {
                let eps = tol.at(rung);
                // The estimate is off by up to a full ε, in either direction — the worst case the
                // monotone ladder allows, sampled rather than assumed.
                let offset = (s.fork("err").below(2_001) as f64 / 1_000.0 - 1.0) * eps;
                let estimate = truth + offset;

                let coarse = within(estimate, limit, eps);
                let exact = within(truth, limit, tol.at(Fidelity::Geometry));

                if let Some(coarse_yes) = coarse.definite() {
                    definite_at_l2 += 1;
                    if let Some(exact_yes) = exact.definite() {
                        assert_eq!(
                            coarse_yes, exact_yes,
                            "L2 said {coarse} at {rung:?} (est {estimate}, tol {eps}) but L4 says \
                             {exact} (truth {truth}, limit {limit})"
                        );
                    }
                }
            }
        }
        assert!(
            definite_at_l2 > 500,
            "only {definite_at_l2} definite L2 answers — the band swallowed everything and the test \
             proved nothing"
        );
    }

    #[test]
    fn a_wider_band_than_the_error_is_what_makes_it_safe() {
        // The falsification of the test above: if the band were *narrower* than the real error, L2
        // would contradict L4. Shown here so the guarantee reads as earned rather than assumed.
        let truth = 10.0;
        let limit = 10.0;
        let real_error = 1.0;
        let estimate = truth - real_error; // 9.0

        // Honest tolerance: the answer is ambiguous, so nothing is claimed.
        assert_eq!(within(estimate, limit, real_error), Trivalent::Ambiguous);
        // Understated tolerance: a confident YES that exact geometry does not support.
        assert_eq!(within(estimate, limit, 0.1), Trivalent::Yes);
        assert_eq!(within(truth, limit, 0.0), Trivalent::Ambiguous);
    }
}
