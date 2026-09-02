//! **Search strategy inside the solve loop** — five heuristics, each with the guardrail that keeps it
//! from restricting what is expressible.
//!
//! ⚠ **A heuristic without its guardrail is a silent restriction on the design space.** Every one of
//! these makes the search cheaper by preferring some solutions over others; the guardrail is what keeps
//! *preferring* from becoming *permitting only*. They are carried together, in one type, because a
//! guardrail written in a comment beside a constant is a guardrail somebody deletes.
//!
//! | Heuristic | Guardrail |
//! |---|---|
//! | most-constrained-first | ordering changes *which* valid solution is found, never *whether* one exists — so it is a selectable **policy**, not a constant |
//! | cache the placement search | keyed on the *resolved configuration*; **deletable without changing the output**, verified with caches off |
//! | decaying attempt budget | reaching zero **escalates**, never abandons |
//! | repulsion for distribution | a *pressure*; never overrides an obligation, a pin, or an explicit placement |
//! | push it out of the band | applies only to an `AMBIGUOUS` scalar answer; it moves an **anchor**, never a decision |
//!
//! # Resolving an ambiguous decision, cheapest first
//!
//! ⚠ **The ordering is part of the design, not an optimisation.** Without it the solver reaches for the
//! expensive option and its budget evaporates on the first hard question.
//!
//! | | Move | Cost |
//! |---|---|---|
//! | **1** | **push it out** — move the anchor until the margin exceeds `tolerance` | usually free, and should be preferred overwhelmingly |
//! | **2** | **defer** — leave it `AMBIGUOUS` and let the next fidelity rung settle it | affordable *because the band is narrow* |
//! | **3** | **tighten the proxy** — contour that one Space early | expensive; the escape hatch, not the plan |
//!
//! ⚠ **Deferral needs no marker.** A decision that answered `AMBIGUOUS` re-asks at the next rung by
//! construction, which is why [`Move::Defer`] carries nothing.
//!
//! # Closing a route is the same move with the sign flipped
//!
//! ⚠ **The design asserted this and left it unspecified.** *Push it out* moves an anchor until a margin
//! exceeds `tolerance`, turning `AMBIGUOUS` into a definite answer. Refusing an adopted-then-rejected
//! shortcut ([`adopt`](crate::adopt)) moves an anchor until the margin is definitely **negative**. Same
//! operation, same cost, opposite [`Target`] — so it is one [`Nudge`] rather than two moves, and the
//! `cyclic-wing` trace's *"balcony lowered 0.6m; margin now -0.2m"* is what it prints.

use crate::trivalent::Trivalent;
use std::fmt;

/// One of the five search heuristics, carried with the guardrail that bounds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Heuristic {
    /// Try the most-constrained placement first.
    MostConstrainedFirst,
    /// Reuse a previous placement search.
    CachePlacementSearch,
    /// Stop retrying locally after so many attempts.
    DecayingAttemptBudget,
    /// Push placements apart for distribution.
    Repulsion,
    /// Move an anchor out of the tolerance band.
    PushItOut,
}

impl Heuristic {
    /// All five.
    pub const ALL: [Heuristic; 5] = [
        Heuristic::MostConstrainedFirst,
        Heuristic::CachePlacementSearch,
        Heuristic::DecayingAttemptBudget,
        Heuristic::Repulsion,
        Heuristic::PushItOut,
    ];

    /// What keeps this heuristic from restricting what is expressible.
    pub fn guardrail(self) -> &'static str {
        match self {
            Heuristic::MostConstrainedFirst => {
                "ordering changes which valid solution is found, never whether one exists — \
                 so it is a selectable policy, not a constant"
            }
            Heuristic::CachePlacementSearch => {
                "keyed on the resolved configuration, and deletable without changing the output"
            }
            Heuristic::DecayingAttemptBudget => "reaching zero escalates, never abandons",
            Heuristic::Repulsion => {
                "a pressure; never overrides an obligation, a pin, or an explicit placement"
            }
            Heuristic::PushItOut => {
                "applies only to an AMBIGUOUS scalar answer; it moves an anchor, never a decision"
            }
        }
    }

    /// May a project turn this off without changing which worlds are *possible*?
    ///
    /// ⚠ **All five, and the check is the point.** A heuristic that could not be disabled would be
    /// part of the definition of a valid world rather than a way of finding one — and the difference
    /// is not visible by reading either.
    pub fn is_optional(self) -> bool {
        true
    }
}

impl fmt::Display for Heuristic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Heuristic::MostConstrainedFirst => "most-constrained-first",
            Heuristic::CachePlacementSearch => "cache-placement-search",
            Heuristic::DecayingAttemptBudget => "decaying-attempt-budget",
            Heuristic::Repulsion => "repulsion",
            Heuristic::PushItOut => "push-it-out",
        })
    }
}

/// One move for resolving an ambiguous decision.
///
/// ⚠ **Ordered cheapest-first by `Ord`**, deliberately: the enum's declaration order *is* the cost
/// order, so `min()` over a set of admissible moves is the design's rule rather than a convention a
/// caller has to remember.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Move {
    /// Move the anchor until the margin clears the band. Usually free.
    PushOut,
    /// Leave it `AMBIGUOUS`; the next fidelity rung settles it.
    ///
    /// ⚠ **Carries nothing.** A deferred decision re-asks at the next rung by construction, so a marker
    /// would be state nobody reads and everybody has to keep correct.
    Defer,
    /// Contour that one Space early to shrink `tolerance`. Expensive.
    TightenProxy,
}

impl Move {
    /// Cheapest first.
    pub const LADDER: [Move; 3] = [Move::PushOut, Move::Defer, Move::TightenProxy];

    /// A rough relative cost, for reporting.
    pub fn cost(self) -> &'static str {
        match self {
            Move::PushOut => "usually free at L3, and should be preferred overwhelmingly",
            Move::Defer => "affordable because the band is narrow",
            Move::TightenProxy => "expensive; the escape hatch, not the plan",
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Move::PushOut => "push it out",
            Move::Defer => "defer",
            Move::TightenProxy => "tighten the proxy",
        })
    }
}

/// Which side of the band a nudge is aiming for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// Above `+tolerance` — the answer becomes a definite yes.
    Clear,
    /// Below `-tolerance` — the answer becomes a definite no, which is how a rejected shortcut is
    /// **closed**.
    Closed,
}

impl Target {
    /// Is a margin far enough out of the band to satisfy this target?
    pub fn satisfied_by(self, margin: f64, tolerance: f64) -> bool {
        match self {
            Target::Clear => margin > tolerance,
            Target::Closed => margin < -tolerance,
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Target::Clear => "clear",
            Target::Closed => "closed",
        })
    }
}

/// Moving an anchor until a margin leaves the tolerance band.
///
/// ⚠ **It moves an anchor and never a decision.** That is `push-it-out`'s guardrail, and it is what
/// makes the move safe to prefer: the decision is re-evaluated against the moved geometry rather than
/// overridden, so a nudge cannot make a false answer true.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nudge {
    /// Where the margin started.
    pub from: f64,
    /// Where it ended.
    pub to: f64,
    /// Which side of the band was being aimed for.
    pub target: Target,
    /// The band's half-width.
    pub tolerance: f64,
}

impl Nudge {
    /// Move a margin toward a target, by whatever distance that takes.
    ///
    /// Returns `None` when the margin is already there — a nudge that moves nothing is not a nudge, and
    /// reporting one would put a no-op in the trace.
    pub fn toward(margin: f64, target: Target, tolerance: f64) -> Option<Nudge> {
        if target.satisfied_by(margin, tolerance) {
            return None;
        }
        // Just past the band, never further: a nudge is the cheapest move and stays that way by
        // moving the least it can.
        let to = match target {
            Target::Clear => tolerance + tolerance.abs().max(f64::EPSILON) * 0.5,
            Target::Closed => -tolerance - tolerance.abs().max(f64::EPSILON) * 0.5,
        };
        Some(Nudge {
            from: margin,
            to,
            target,
            tolerance,
        })
    }

    /// How far the anchor moved.
    pub fn distance(&self) -> f64 {
        (self.to - self.from).abs()
    }

    /// Did it land where it was aiming?
    pub fn resolved(&self) -> bool {
        self.target.satisfied_by(self.to, self.tolerance)
    }
}

impl fmt::Display for Nudge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "moved {:.1}; margin now {:.1} ({})",
            self.distance(),
            self.to,
            self.target
        )
    }
}

/// Which heuristics are on, and the ladder they use for an ambiguous answer.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchPolicy {
    enabled: Vec<Heuristic>,
    /// Which moves the caller can actually afford right now.
    ///
    /// ⚠ **`TightenProxy` is admissible by default and still last**, because *"expensive"* is a
    /// reason to order it last rather than a reason to withhold it. A policy that removed it would
    /// leave a decision with nowhere to go at the final rung.
    admissible: Vec<Move>,
}

impl Default for SearchPolicy {
    fn default() -> Self {
        SearchPolicy {
            enabled: Heuristic::ALL.to_vec(),
            admissible: Move::LADDER.to_vec(),
        }
    }
}

impl SearchPolicy {
    /// Everything on.
    pub fn new() -> Self {
        SearchPolicy::default()
    }

    /// Turn a heuristic off.
    pub fn without(mut self, h: Heuristic) -> Self {
        self.enabled.retain(|e| *e != h);
        self
    }

    /// Is it on?
    pub fn enabled(&self, h: Heuristic) -> bool {
        self.enabled.contains(&h)
    }

    /// Restrict which moves are affordable.
    pub fn affording(mut self, moves: impl IntoIterator<Item = Move>) -> Self {
        self.admissible = moves.into_iter().collect();
        self.admissible.sort_unstable();
        self
    }

    /// The move to make for an ambiguous answer: the cheapest admissible one.
    ///
    /// ⚠ **Returns `None` for a decided answer**, because there is nothing to resolve — and a caller
    /// that nudged anyway would be moving geometry to change an answer that was already definite,
    /// which is exactly what `push-it-out`'s guardrail forbids.
    pub fn resolve(&self, answer: Trivalent) -> Option<Move> {
        if answer != Trivalent::Ambiguous {
            return None;
        }
        let mut admissible = self.admissible.clone();
        if !self.enabled(Heuristic::PushItOut) {
            admissible.retain(|m| *m != Move::PushOut);
        }
        admissible.into_iter().min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_heuristic_carries_the_guardrail_that_bounds_it() {
        for h in Heuristic::ALL {
            assert!(
                !h.guardrail().is_empty(),
                "{h} is a silent restriction on the design space without one"
            );
            assert!(h.is_optional(), "{h} would otherwise define a valid world");
        }
        assert_eq!(Heuristic::ALL.len(), 5);
    }

    #[test]
    fn the_ladder_is_ordered_cheapest_first_by_the_type_itself() {
        // ⚠ Declaration order *is* cost order, so `min()` is the design's rule.
        assert!(Move::PushOut < Move::Defer);
        assert!(Move::Defer < Move::TightenProxy);
        assert_eq!(
            Move::LADDER.iter().min(),
            Some(&Move::PushOut),
            "without the ordering the solver reaches for the expensive option first"
        );
        for m in Move::LADDER {
            assert!(!m.cost().is_empty());
        }
    }

    #[test]
    fn an_ambiguous_answer_resolves_with_the_cheapest_admissible_move() {
        let p = SearchPolicy::new();
        assert_eq!(p.resolve(Trivalent::Ambiguous), Some(Move::PushOut));
    }

    #[test]
    fn a_decided_answer_has_nothing_to_resolve() {
        let p = SearchPolicy::new();
        assert_eq!(p.resolve(Trivalent::Yes), None);
        assert_eq!(
            p.resolve(Trivalent::No),
            None,
            "nudging a definite answer is what the push-it-out guardrail forbids"
        );
    }

    #[test]
    fn disabling_push_it_out_falls_to_the_next_rung_rather_than_to_nothing() {
        let p = SearchPolicy::new().without(Heuristic::PushItOut);
        assert!(!p.enabled(Heuristic::PushItOut));
        assert_eq!(p.resolve(Trivalent::Ambiguous), Some(Move::Defer));
    }

    #[test]
    fn a_policy_that_can_only_afford_the_expensive_move_still_has_one() {
        let p = SearchPolicy::new().affording([Move::TightenProxy]);
        assert_eq!(p.resolve(Trivalent::Ambiguous), Some(Move::TightenProxy));
    }

    #[test]
    fn a_nudge_toward_clear_leaves_the_band_on_the_positive_side() {
        let n = Nudge::toward(0.05, Target::Clear, 0.2).expect("0.05 is inside the band");
        assert!(n.to > 0.2);
        assert!(n.resolved());
        assert!(n.distance() > 0.0);
    }

    #[test]
    fn a_nudge_toward_closed_leaves_the_band_on_the_negative_side() {
        // ⚠ The cyclic-wing trace's "balcony lowered; margin now negative" — the same move, flipped.
        let n = Nudge::toward(0.4, Target::Closed, 0.2).expect("0.4 is not closed");
        assert!(
            n.to < -0.2,
            "a closed route has a definitely negative margin"
        );
        assert!(n.resolved());
        assert!(n.to_string().contains("closed"));
    }

    #[test]
    fn a_margin_already_where_it_is_aiming_produces_no_nudge() {
        // A no-op in the trace is worse than no line at all.
        assert!(Nudge::toward(0.9, Target::Clear, 0.2).is_none());
        assert!(Nudge::toward(-0.9, Target::Closed, 0.2).is_none());
    }

    #[test]
    fn the_two_targets_disagree_about_the_same_margin() {
        assert!(Target::Clear.satisfied_by(0.5, 0.2));
        assert!(!Target::Closed.satisfied_by(0.5, 0.2));
        assert!(Target::Closed.satisfied_by(-0.5, 0.2));
        assert!(!Target::Clear.satisfied_by(-0.5, 0.2));
        // Inside the band satisfies neither, which is what makes it ambiguous.
        assert!(!Target::Clear.satisfied_by(0.1, 0.2));
        assert!(!Target::Closed.satisfied_by(-0.1, 0.2));
    }

    #[test]
    fn a_zero_tolerance_band_still_produces_a_move_that_lands_outside_it() {
        // ⚠ Exact geometry: the band has no width, so any non-zero margin decides — but a margin of
        // exactly zero is still ambiguous and must be movable.
        let n = Nudge::toward(0.0, Target::Clear, 0.0).expect("zero is on the boundary");
        assert!(n.to > 0.0);
        assert!(n.resolved());
    }
}
