//! M07 exit criteria: **a `judge()` gradient drives a search that converges rather than rerolling.**
//!
//! This is the claim the whole verdict design rests on, and it is not provable by checking that
//! `Verdict` has four variants. The test is behavioural:
//!
//! > Given a mechanic that reports *how far off* a candidate was, a naive solver must reach a fit in
//! > far fewer attempts than one that only learns *yes* or *no*.
//!
//! ⚠ **The control matters more than the result.** A converging search proves nothing on its own —
//! maybe the problem was easy. It proves something when the *same search*, given only a boolean,
//! visibly fails to converge on the *same problem*.
//!
//! # The three concreteness levels
//!
//! The loop runs at L2, L3 and L4 against progressively firmer geometry. The design's requirement is
//! that a proposal's *shape* does not change between them — only its precision — so the same mechanic
//! answers all three without knowing which it is in.

use cv_core::judge::{Budget, Verdict};
use cv_core::{Fidelity, Tolerances, Trivalent};
use cv_determinism::Rng;

/// A tether mechanic: it accepts a gap it can span, and reports the shortfall when it cannot.
///
/// This is the `ranged-traversal` scenario's `judge()` in miniature — the gradient the milestone's
/// green criterion is about.
struct Tether {
    reach: f64,
}

impl Tether {
    /// ⚠ **The magnitude is the whole point.** `OverBudget(excess)` says *move it this much closer*.
    fn judge(&self, gap: f64) -> Verdict {
        Budget::distance(self.reach).judge(gap)
    }

    /// The same mechanic, deliberately crippled: it answers only yes or no.
    ///
    /// Not a straw man — this is exactly what a `-> bool` hook would give the solver.
    fn judge_boolean(&self, gap: f64) -> bool {
        gap <= self.reach
    }
}

/// A solver that uses the magnitude: move the target closer by what the verdict reported.
///
/// ⚠ **Modelled with placement noise on purpose.** A solver that closed the reported excess exactly
/// would converge in one step and prove only that subtraction works. Real placement lands *near* the
/// requested spot, so the step is the reported excess scaled by 0.7–1.3 — and convergence then has to
/// come from the feedback loop rather than from arithmetic.
fn search_with_gradient(mech: &Tether, mut gap: f64, budget: usize, rng: &Rng) -> Option<usize> {
    for attempt in 1..=budget {
        match mech.judge(gap) {
            v if v.is_accepted() => return Some(attempt),
            v => {
                let excess = v.shortfall()?;
                let noise = 0.7 + rng.fork_index(attempt as u64).below(601) as f64 / 1_000.0;
                gap -= excess * noise;
            }
        }
    }
    None
}

/// The same solver given only a boolean: it can do nothing but try somewhere else.
fn search_by_reroll(mech: &Tether, gap: f64, budget: usize, rng: &Rng) -> Option<usize> {
    for attempt in 1..=budget {
        // With no magnitude there is no direction to move in, so the only move is another guess.
        let candidate = rng.fork_index(attempt as u64).below(10_000) as f64 / 100.0;
        if mech.judge_boolean(candidate) {
            return Some(attempt);
        }
        let _ = gap;
    }
    None
}

#[test]
fn a_gradient_converges_where_a_boolean_rerolls() {
    // ⚠ **The milestone's green criterion.** A hard case: the gap is far outside reach, and the
    // acceptable band is a narrow slice of the search space.
    let mech = Tether { reach: 30.0 };
    let gap = 96.0;

    let rng = Rng::new(0xC0FFEE);
    let converged =
        search_with_gradient(&mech, gap, 40, &rng).expect("the gradient must reach a fit");
    assert!(
        converged <= 10,
        "a search told *how far off* should close quickly, took {converged}"
    );

    // The control. Same mechanic, same problem, no magnitude.
    let rerolls: Vec<Option<usize>> = (0..8)
        .map(|s| search_by_reroll(&mech, gap, 10, &rng.fork_index(s)))
        .collect();
    let worst = rerolls.iter().filter(|r| r.is_none()).count();
    assert!(
        worst > 0,
        "the boolean control must sometimes fail within the same budget, or this proves nothing"
    );
}

#[test]
fn the_gradient_shrinks_monotonically() {
    // Convergence, stated directly: each rejection must be less severe than the one before it.
    let mech = Tether { reach: 30.0 };
    let mut gap = 200.0;
    let mut last = f64::INFINITY;
    let mut steps = 0;

    // A conservative solver: it closes 90% of what it was told, every time.
    while let Some(excess) = mech.judge(gap).shortfall() {
        assert!(
            excess < last,
            "step {steps}: shortfall grew from {last} to {excess} — the search is diverging"
        );
        last = excess;
        gap -= excess * 0.9;
        steps += 1;
        assert!(steps < 200, "did not converge");
    }
    assert!(mech.judge(gap).is_accepted());
}

#[test]
fn an_unsuitable_candidate_stops_the_search_instead_of_looping() {
    // ⚠ The termination condition. Without `Unsuitable`, *wrong kind of thing* is indistinguishable
    // from *wrong place*, and the loop re-offers a doomed candidate until its budget runs out.
    let v = Verdict::unsuitable("a door is not a floor");
    assert!(!v.is_retryable());
    assert_eq!(
        v.shortfall(),
        None,
        "there is no distance that would fix it"
    );
}

#[test]
fn a_blocked_verdict_names_what_to_move() {
    // "Something is in the way" is only actionable if it says *what*.
    let pillar = cv_core::ObjectId::derived("actor", "pillar");
    match Verdict::blocked(pillar) {
        Verdict::Blocked { by } => assert_eq!(by, pillar),
        other => panic!("expected a blockage, got {other}"),
    }
}

// ---------------------------------------------------------------------------------------------
// The three concreteness levels
// ---------------------------------------------------------------------------------------------

#[test]
fn the_same_mechanic_answers_at_every_rung_without_knowing_which() {
    // ⚠ **A proposal's shape does not change between L2, L3 and L4 — only its precision.** If the
    // mechanic had to know which rung it was on, every authored hook would carry a fidelity switch,
    // and the pipeline could never refine a decision without re-authoring it.
    let mech = Tether { reach: 30.0 };
    let tol = Tolerances::default();

    // The true gap is comfortably inside reach; each rung measures it with less error.
    let truth = 22.0;
    let mut previous_uncertainty = f64::INFINITY;
    for rung in Fidelity::ALL {
        let eps = tol.at(rung);
        assert!(
            eps < previous_uncertainty,
            "the ladder must only ever tighten"
        );
        previous_uncertainty = eps;

        // The mechanic's own answer never changes shape.
        assert!(mech.judge(truth).is_accepted(), "{rung:?}");
    }
}

#[test]
fn a_decision_inside_the_band_defers_rather_than_guessing() {
    // ⚠ The deferral path, exercised end to end: an `AMBIGUOUS` answer re-asks at the next rung, and
    // the answer sharpens because the band shrank — not because anything was re-authored.
    let tol = Tolerances::default();
    let limit = 30.0;
    let measured = 30.3; // inside the envelope's band, outside the hull's

    assert_eq!(
        cv_core::within(measured, limit, tol.at(Fidelity::Envelope)),
        Trivalent::Ambiguous,
        "at the coarsest rung this genuinely cannot be answered"
    );
    assert_eq!(
        cv_core::within(measured, limit, tol.at(Fidelity::Hull)),
        Trivalent::No,
        "one rung down the band is narrow enough to settle it"
    );
}

#[test]
fn rejection_is_the_channel_and_not_an_error_path() {
    // ⚠ An implementation that logged, retried or swallowed rejections would have inverted the
    // design. Every rejection here carries information the search consumed to get closer.
    let mech = Tether { reach: 30.0 };
    let mut gap = 120.0;
    let mut rejections = 0;

    while let Some(excess) = mech.judge(gap).shortfall() {
        rejections += 1;
        gap -= excess * 0.9;
        if rejections > 200 {
            panic!("did not converge");
        }
    }
    assert!(
        rejections >= 3,
        "the search should have been refused several times on the way to a fit, got {rejections}"
    );
    assert!(mech.judge(gap).is_accepted());
}
