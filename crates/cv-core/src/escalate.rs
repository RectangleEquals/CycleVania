//! **Escalation** — what happens when a layer cannot satisfy what it was asked for.
//!
//! # P6: substitution, never removal
//!
//! ⚠ **Dropping content is the last resort, and it is always reported.** The response to an
//! unsatisfiable constraint is to **substitute something that satisfies it**, or to **escalate to a
//! layer that can fix it**. A generator that quietly omitted whatever it found hard would produce a
//! world that passes every check and is missing the thing the developer asked for — the failure mode
//! this whole project is arranged against.
//!
//! # A decaying attempt budget escalates; it never abandons
//!
//! ⚠ **This is the guardrail on the fourth search heuristic**, and it inverts what a retry budget
//! normally means. Reaching zero is not *"give up on this placement"* — it is *"stop retrying **here**;
//! ask the layer that can actually fix this."* Those are opposite instructions, and a budget that meant
//! the first would silently thin out a world under exactly the conditions where content matters most.
//!
//! Local retries are cheap and get cheaper as the search narrows; the escalation is expensive and rare.
//! So the budget is not a limit on effort, it is **a limit on effort spent at the wrong level**.
//!
//! # The ladder
//!
//! Each failure escalates to the layer that owns the thing that would have to change, and that layer's
//! permissions are bounded — otherwise *"escalate"* becomes *"anything may now be rewritten"*, and a
//! deterministic pipeline stops being one.
//!
//! | Failure | Escalates to | May change |
//! |---|---|---|
//! | no candidate satisfies a `Constraint` | **L1 Mission** | where the gate sits; which location holds the item |
//! | a `PlacementNeed` finds no site | **L3 Volume** | the envelope, the anchor, the clearance around it |
//! | a `GUARDED` gate reports `Unproven` | **L3 Volume** | the fidelity the question is asked at |
//! | a `GUARDED` gate reports `Breached` | **L4 Geometry** | the offending geometry, nudged until the route closes |
//! | the content pool cannot supply a role | **L0 Content** | nothing — this is authoring, and it is reported to the developer |
//!
//! ⚠ **L0 can change nothing, and that is the point.** A generator that invented content to fill a gap
//! would be authoring, which is the developer's job. The escalation terminates in a **report**, which
//! is the honest end of the ladder rather than a failure of it.

use crate::node::NodeKind;
use crate::object::ObjectId;
use std::fmt;

/// A pipeline layer, as an escalation target.
///
/// ⚠ **L2 Skeleton is absent on purpose.** Nothing escalates *to* it: it consumes the mission graph and
/// produces the scope tree, and every repair a failure could want is either a mission decision (L1) or
/// a volume one (L3). Listing it for symmetry would invite a caller to escalate somewhere with no
/// permission to act.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// L0 — authored content. Changes nothing; reports to the developer.
    Content,
    /// L1 — the mission graph, spheres and gates.
    Mission,
    /// L3 — envelopes, anchors and clearance.
    Volume,
    /// L4 — realized geometry.
    Geometry,
}

impl Layer {
    /// What this layer is permitted to change in response to an escalation.
    ///
    /// ⚠ **Stated, because *"escalate"* without a permission is *"rewrite anything"*.** A ladder whose
    /// rungs have no bounds is not a ladder.
    pub fn may_change(self) -> &'static str {
        match self {
            Layer::Content => "nothing — this is authoring, and it is reported to the developer",
            Layer::Mission => "where a gate sits; which location holds an item",
            Layer::Volume => "the envelope, the anchor, the clearance around it, the fidelity",
            Layer::Geometry => "the offending geometry, nudged until the route closes",
        }
    }

    /// Can this layer act at all, or does the ladder terminate in a report?
    pub fn can_repair(self) -> bool {
        self != Layer::Content
    }

    /// The label a trace line uses.
    pub fn tag(self) -> &'static str {
        match self {
            Layer::Content => "L0",
            Layer::Mission => "L1",
            Layer::Volume => "L3",
            Layer::Geometry => "L4",
        }
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

/// Why something could not be satisfied locally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// No candidate location satisfied the constraints.
    NoCandidate { constraint: String },
    /// A placement need found no site.
    NoSite { need: String },
    /// A guarded gate could not be proven exclusive at this fidelity.
    Unproven { edge: usize },
    /// A guarded gate has an alternative route.
    Breached { edge: usize },
    /// The content pool has nothing that can fill a role.
    NoSupply { role: String },
}

impl Failure {
    /// The layer that owns whatever would have to change.
    pub fn escalates_to(&self) -> Layer {
        match self {
            Failure::NoCandidate { .. } => Layer::Mission,
            Failure::NoSite { .. } => Layer::Volume,
            Failure::Unproven { .. } => Layer::Volume,
            Failure::Breached { .. } => Layer::Geometry,
            Failure::NoSupply { .. } => Layer::Content,
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::NoCandidate { constraint } => {
                write!(f, "no candidate satisfies {constraint}")
            }
            Failure::NoSite { need } => write!(f, "no site satisfies {need}"),
            Failure::Unproven { edge } => {
                write!(f, "gate on edge {edge} cannot be proven exclusive here")
            }
            Failure::Breached { edge } => write!(f, "gate on edge {edge} has an alternative route"),
            Failure::NoSupply { role } => write!(f, "the content pool supplies no {role}"),
        }
    }
}

/// What the generator did about a failure.
///
/// ⚠ **`Dropped` is a variant and not an absence**, which is the whole of P6 in one type. A response
/// enum without it would have forced the drop path to be *no response at all* — unrepresentable, and
/// therefore unreportable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
    /// Something else that satisfies the requirement went in instead.
    Substituted { instead: ObjectId },
    /// Handed to a layer that can change what is in the way.
    Escalated { to: Layer },
    /// ⚠ **The last resort.** Always reported; never silent.
    Dropped { why: String },
}

impl Response {
    /// Did content actually go in?
    pub fn placed_something(&self) -> bool {
        matches!(self, Response::Substituted { .. })
    }

    /// Must this appear in the developer-facing report?
    ///
    /// ⚠ **All three, and that is deliberate.** A substitution is not the thing that was asked for; an
    /// escalation changed a decision at another layer; a drop removed content. There is no response
    /// here quiet enough to omit.
    pub fn is_reportable(&self) -> bool {
        true
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Response::Substituted { .. } => write!(f, "substituted"),
            Response::Escalated { to } => write!(f, "escalated to {to} ({})", to.may_change()),
            Response::Dropped { why } => write!(f, "DROPPED — {why}"),
        }
    }
}

/// One row of the escalations-and-substitutions report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Escalation {
    /// What could not be done.
    pub failure: Failure,
    /// What was done about it.
    pub response: Response,
    /// How many local attempts were spent before this.
    pub attempts: u32,
    /// Where, when a scope kind is meaningful.
    pub scope: Option<NodeKind>,
}

impl fmt::Display for Escalation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} after {} attempt(s) — {}",
            self.failure, self.attempts, self.response
        )
    }
}

/// A decaying budget for local retries.
///
/// ⚠ **Reaching zero escalates.** See the module header: this is a limit on effort spent at the wrong
/// level, not a limit on effort.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptBudget {
    remaining: u32,
    spent: u32,
    initial: u32,
}

impl AttemptBudget {
    /// A budget of this many local attempts.
    pub fn new(attempts: u32) -> Self {
        AttemptBudget {
            remaining: attempts,
            spent: 0,
            initial: attempts,
        }
    }

    /// Spend one attempt. Answers whether another local try is allowed.
    pub fn attempt(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        self.spent += 1;
        self.remaining > 0
    }

    /// Is local retrying finished?
    pub fn exhausted(&self) -> bool {
        self.remaining == 0
    }

    /// How many attempts have been spent.
    pub fn spent(&self) -> u32 {
        self.spent
    }

    /// How many remain.
    pub fn remaining(&self) -> u32 {
        self.remaining
    }

    /// Start over — a new placement gets the full budget.
    pub fn reset(&mut self) {
        self.remaining = self.initial;
        self.spent = 0;
    }

    /// The escalation an exhausted budget produces.
    ///
    /// ⚠ **There is no `None` branch here.** An exhausted budget always yields an `Escalation`, which
    /// is what makes *"escalates, never abandons"* a property of the type rather than a promise about
    /// how callers behave.
    pub fn escalate(&self, failure: Failure, scope: Option<NodeKind>) -> Escalation {
        let to = failure.escalates_to();
        Escalation {
            response: if to.can_repair() {
                Response::Escalated { to }
            } else {
                Response::Dropped {
                    why: format!("{failure}; {} may change {}", to, to.may_change()),
                }
            },
            failure,
            attempts: self.spent,
            scope,
        }
    }
}

impl Default for AttemptBudget {
    /// ⚠ **Eight, and the number is arbitrary in a way the *shape* is not.** What matters is that it
    /// is small: a large budget is a slow way to reach the same escalation, and every attempt past the
    /// first few is searching a space the constraints already said was empty.
    fn default() -> Self {
        AttemptBudget::new(8)
    }
}

/// The escalations-and-substitutions report.
///
/// ⚠ **The writer the editor's report has been missing.** `M21 P03` renders this; until now nothing
/// produced it, so the view had a reader and no writer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EscalationReport {
    rows: Vec<Escalation>,
}

impl EscalationReport {
    /// An empty report.
    pub fn new() -> Self {
        EscalationReport::default()
    }

    /// Record one.
    pub fn record(&mut self, e: Escalation) {
        self.rows.push(e);
    }

    /// Every row, in the order they happened.
    pub fn rows(&self) -> &[Escalation] {
        &self.rows
    }

    /// Rows whose response dropped content.
    ///
    /// ⚠ **The one a developer reads first.** A substitution changed what they get; a drop means they
    /// do not get it, and P6 makes that the rarest and loudest outcome.
    pub fn drops(&self) -> Vec<&Escalation> {
        self.rows
            .iter()
            .filter(|e| matches!(e.response, Response::Dropped { .. }))
            .collect()
    }

    /// How many rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Nothing needed reporting.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_failure_escalates_to_a_layer_that_owns_the_fix() {
        let cases = [
            (
                Failure::NoCandidate {
                    constraint: "MinDistanceFrom".into(),
                },
                Layer::Mission,
            ),
            (
                Failure::NoSite {
                    need: "NeedsClearance".into(),
                },
                Layer::Volume,
            ),
            (Failure::Unproven { edge: 1 }, Layer::Volume),
            (Failure::Breached { edge: 1 }, Layer::Geometry),
            (
                Failure::NoSupply {
                    role: "PROGRESSION".into(),
                },
                Layer::Content,
            ),
        ];
        for (f, expect) in cases {
            assert_eq!(f.escalates_to(), expect, "{f}");
        }
    }

    #[test]
    fn an_exhausted_budget_escalates_rather_than_abandoning() {
        let mut b = AttemptBudget::new(3);
        assert!(b.attempt());
        assert!(b.attempt());
        assert!(!b.attempt(), "the third spends the last one");
        assert!(b.exhausted());
        assert_eq!(b.spent(), 3);

        let e = b.escalate(Failure::NoSite { need: "n".into() }, None);
        assert_eq!(
            e.response,
            Response::Escalated { to: Layer::Volume },
            "reaching zero asks the layer that can fix it; it does not give up on the placement"
        );
        assert_eq!(e.attempts, 3);
    }

    #[test]
    fn the_ladder_terminates_in_a_report_rather_than_a_repair() {
        // ⚠ L0 may change nothing: inventing content to fill a gap would be authoring.
        let b = AttemptBudget::new(1);
        let e = b.escalate(
            Failure::NoSupply {
                role: "PROGRESSION".into(),
            },
            None,
        );
        assert!(matches!(e.response, Response::Dropped { .. }));
        assert!(!Layer::Content.can_repair());
        assert!(e.to_string().contains("DROPPED"));
    }

    #[test]
    fn a_drop_always_says_why() {
        let b = AttemptBudget::new(1);
        let e = b.escalate(
            Failure::NoSupply {
                role: "FILLER".into(),
            },
            None,
        );
        let Response::Dropped { why } = &e.response else {
            panic!("expected a drop");
        };
        assert!(!why.is_empty(), "a silent drop is the thing P6 forbids");
        assert!(why.contains("FILLER"));
    }

    #[test]
    fn every_response_is_reportable() {
        // ⚠ There is no response quiet enough to omit — see `Response::is_reportable`.
        for r in [
            Response::Substituted {
                instead: ObjectId::derived("item", "x"),
            },
            Response::Escalated { to: Layer::Volume },
            Response::Dropped { why: "w".into() },
        ] {
            assert!(r.is_reportable());
        }
    }

    #[test]
    fn a_substitution_places_something_and_the_other_two_do_not() {
        assert!(Response::Substituted {
            instead: ObjectId::derived("item", "x")
        }
        .placed_something());
        assert!(!Response::Escalated { to: Layer::Mission }.placed_something());
        assert!(!Response::Dropped { why: "w".into() }.placed_something());
    }

    #[test]
    fn a_reset_budget_starts_over() {
        let mut b = AttemptBudget::new(2);
        b.attempt();
        b.attempt();
        assert!(b.exhausted());
        b.reset();
        assert!(!b.exhausted());
        assert_eq!(b.spent(), 0);
        assert_eq!(b.remaining(), 2);
    }

    #[test]
    fn a_zero_budget_escalates_without_ever_trying_locally() {
        let mut b = AttemptBudget::new(0);
        assert!(!b.attempt());
        assert!(b.exhausted());
        assert_eq!(b.spent(), 0);
    }

    #[test]
    fn the_report_surfaces_drops_ahead_of_everything_else() {
        let mut r = EscalationReport::new();
        let b = AttemptBudget::new(1);
        r.record(b.escalate(Failure::NoSite { need: "n".into() }, None));
        r.record(b.escalate(
            Failure::NoSupply {
                role: "LANDMARK".into(),
            },
            None,
        ));
        assert_eq!(r.len(), 2);
        assert_eq!(r.drops().len(), 1);
        assert!(!r.is_empty());
    }

    #[test]
    fn every_layer_states_what_it_may_change() {
        for l in [
            Layer::Content,
            Layer::Mission,
            Layer::Volume,
            Layer::Geometry,
        ] {
            assert!(
                !l.may_change().is_empty(),
                "an escalation target with no stated permission is `rewrite anything`"
            );
            assert!(!l.tag().is_empty());
        }
    }
}
