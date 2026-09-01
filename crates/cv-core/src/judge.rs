//! **Verdicts, budgets, routes and paths** — the propose-and-judge currency.
//!
//! # Why a verdict is not a bool
//!
//! > **A boolean would tell the solver nothing and turn the placement search into a reroll.**
//!
//! When a mechanic rejects a candidate, the useful part is *by how much*. `OverBudget(6.2)` says
//! **move it 6.2 closer** — a direction the search can act on, converging on a fit. `false` says only
//! *try again*, which is a random walk wearing a search's clothes.
//!
//! ⚠ **A reject or redirect is expected and is not an error path.** It is how a mechanic's knowledge —
//! which the core cannot have — reaches the placement decision. An implementation that logs, retries or
//! swallows a rejection has inverted the design: **rejection is the channel, not the exception.**
//!
//! # The four verdicts, and what each tells the search to do next
//!
//! | Verdict | The search should |
//! |---|---|
//! | `Accepted(slack)` | commit — and the slack feeds difficulty, because *barely fits* and *fits easily* are different worlds |
//! | `OverBudget(excess)` | move the target closer by roughly that much |
//! | `Blocked(by)` | remove or reposition **that**, or route around it |
//! | `Unsuitable(reason)` | **stop retrying this candidate.** Nothing about position will fix it |
//!
//! That last distinction is what keeps the search finite. Without it, *wrong kind of thing* is
//! indistinguishable from *wrong place*, and the solver re-offers the same doomed candidate forever.
//!
//! # Routes oblige; spans merely describe
//!
//! > **The sign is in the primitive.** A `Span` *declares* a range. A [`Route`] *obliges* one.
//!
//! A route is required or forbidden, and either way the solver must act on it. That is why forbidden
//! routes live here rather than as a flag somewhere: *"the boss must not be reachable from the entrance
//! without the key"* is the same kind of statement as *"these two rooms must connect"*, and giving them
//! one shape is what lets one search satisfy both.

use crate::object::ObjectId;
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use cv_determinism::Vec3;
use std::fmt;

// ---------------------------------------------------------------------------------------------
// Verdict
// ---------------------------------------------------------------------------------------------

/// What `judge()` returns.
///
/// ⚠ **The magnitude is the point.** Every rejecting variant carries the quantity the solver needs to
/// do something other than guess again.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// It fits. `slack` says how comfortably, which feeds difficulty.
    ///
    /// ⚠ Zero slack is still `Accepted` — *barely* is a fit. It is also the most fragile kind, which
    /// is why the number survives into the trace rather than being thresholded here.
    Accepted { slack: f64 },
    /// Too far, by roughly this much. **A direction the solver can act on.**
    ///
    /// ⚠ `against` names the budget it was measured against, and it is what turns *"over budget by
    /// 6.2"* — a number with no referent — into *"over budget by 6.2 against grapple reach"*. For a
    /// developer asking *"why did this fail?"* that name is the single most useful fact, and it is
    /// free once budgets have names. `None` only where the comparison was against a bare number.
    OverBudget {
        excess: f64,
        against: Option<ObjectId>,
    },
    /// Something is in the way. Remove or reposition it, or route around it.
    Blocked { by: ObjectId },
    /// Wrong kind of thing.
    ///
    /// ⚠ **Do not retry this candidate.** No amount of repositioning will help, and treating this as
    /// *over budget by zero* is how a search stops terminating.
    Unsuitable { reason: String },
}

impl Verdict {
    /// A fit with the given slack.
    pub fn accepted(slack: f64) -> Self {
        Verdict::Accepted { slack }
    }

    /// Over budget by `excess`.
    ///
    /// ⚠ A non-positive excess is not a rejection — it is a fit, and is returned as one. Letting
    /// `OverBudget(0.0)` exist would give the solver a rejection it cannot act on.
    pub fn over_budget(excess: f64) -> Self {
        if excess <= 0.0 {
            Verdict::Accepted { slack: -excess }
        } else {
            Verdict::OverBudget {
                excess,
                against: None,
            }
        }
    }

    /// Name the budget this was measured against.
    ///
    /// ⚠ Chaining rather than a constructor argument, so an `Accepted` verdict silently absorbs it —
    /// a fit has nothing to attribute, and forcing every caller to branch on that would put the same
    /// `if` at every call site.
    pub fn against(mut self, budget: ObjectId) -> Self {
        if let Verdict::OverBudget { against, .. } = &mut self {
            *against = Some(budget);
        }
        self
    }

    /// Which budget this was measured against, if it was a rejection that names one.
    pub fn budget(&self) -> Option<ObjectId> {
        match self {
            Verdict::OverBudget { against, .. } => *against,
            _ => None,
        }
    }

    /// Blocked by a specific thing.
    pub fn blocked(by: ObjectId) -> Self {
        Verdict::Blocked { by }
    }

    /// Wrong kind of thing, with a reason a human can read.
    pub fn unsuitable(reason: impl Into<String>) -> Self {
        Verdict::Unsuitable {
            reason: reason.into(),
        }
    }

    /// Did it fit?
    pub fn is_accepted(&self) -> bool {
        matches!(self, Verdict::Accepted { .. })
    }

    /// Is retrying this candidate at another position worth anything?
    ///
    /// ⚠ The search's termination condition. `Unsuitable` is the only verdict that says *no*.
    pub fn is_retryable(&self) -> bool {
        !matches!(self, Verdict::Unsuitable { .. })
    }

    /// How far off this was, for a solver deciding what to move and by how much.
    ///
    /// `None` where the answer is not a distance — a blockage and a category error are both real
    /// rejections, and neither is measured in metres.
    pub fn shortfall(&self) -> Option<f64> {
        match self {
            Verdict::OverBudget { excess, .. } => Some(*excess),
            Verdict::Accepted { .. } | Verdict::Blocked { .. } | Verdict::Unsuitable { .. } => None,
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Accepted { slack } => write!(f, "accepted with {slack} to spare"),
            Verdict::OverBudget {
                excess,
                against: Some(b),
            } => write!(f, "over budget by {excess} against {b}"),
            Verdict::OverBudget {
                excess,
                against: None,
            } => write!(f, "over budget by {excess}"),
            Verdict::Blocked { by } => write!(f, "blocked by {by}"),
            Verdict::Unsuitable { reason } => write!(f, "unsuitable: {reason}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Budgets live in their own module
// ---------------------------------------------------------------------------------------------

// ⚠ **`Budget` moved to [`crate::budget`], and it is not a rename.** It used to fuse the *declaration*
// ("at most 8 metres") with the *accounting* ("6.2 spent"), so every rule and route that named a limit
// owned a private copy of it — and a project retuning "carry range" edited twelve sites and missed one.
// A budget is now a named row a project retunes in one place, and the name survives into the verdict.
pub use crate::budget::{Budget, BudgetBook, BudgetError, BudgetRef, Cost};

// ---------------------------------------------------------------------------------------------
// Path
// ---------------------------------------------------------------------------------------------

/// One leg of a realised path.
#[derive(Clone, Debug, PartialEq)]
pub struct PathStep {
    /// Where this leg ends.
    pub position: Vec3,
    /// How long it is.
    pub length: f64,
    /// The surface travelled, if the generator knows it yet.
    pub surface: Option<ObjectId>,
    /// The floor this leg is on, if it is on one.
    pub floor: Option<ObjectId>,
    /// What made this leg possible — a staircase, a rope, an ability.
    pub via: Option<ObjectId>,
}

/// A realised route.
///
/// ⚠ **This is what a [`Route`] becomes once the generator has produced it, and it is the reason there
/// is no spline resource.** A developer declares the *obligation*; the generator produces the *shape*.
/// An authored curve-through-space would need geometry that does not exist at authoring time.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    steps: Vec<PathStep>,
    origin: Vec3,
}

impl Path {
    /// A path starting at a point.
    pub fn from(origin: Vec3) -> Self {
        Path {
            steps: Vec::new(),
            origin,
        }
    }

    /// Extend the path to a point.
    pub fn step_to(mut self, position: Vec3) -> Self {
        let last = self.steps.last().map(|s| s.position).unwrap_or(self.origin);
        self.steps.push(PathStep {
            position,
            length: (position - last).length(),
            surface: None,
            floor: None,
            via: None,
        });
        self
    }

    /// The legs, in order.
    pub fn steps(&self) -> &[PathStep] {
        &self.steps
    }

    /// Where it starts.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    /// Where it ends — the origin if it has no steps.
    pub fn target(&self) -> Vec3 {
        self.steps.last().map(|s| s.position).unwrap_or(self.origin)
    }

    /// Total travelled length, summing legs rather than measuring end to end.
    ///
    /// ⚠ A path that doubles back is *longer* than the distance between its ends, and a budget is
    /// spent on the travelling, not on the displacement.
    pub fn length(&self) -> f64 {
        self.steps.iter().map(|s| s.length).sum()
    }

    /// Net vertical change from origin to target.
    ///
    /// Signed on purpose: descending is not the same as climbing, and a jump budget cares which.
    pub fn rise(&self) -> f64 {
        self.target().y - self.origin.y
    }
}

// ---------------------------------------------------------------------------------------------
// Route
// ---------------------------------------------------------------------------------------------

/// Required, or forbidden.
///
/// ⚠ **The sign is in the primitive.** A forbidden route is not a required route with a flag — it is a
/// different obligation, and the solver satisfies it by making sure no such path exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Obligation {
    /// This path must exist.
    Required,
    /// This path must **not** exist.
    ///
    /// *"The boss must not be reachable from the entrance without the key."*
    Forbidden,
}

/// A required or forbidden path, with a budget and predicates.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    /// Where it starts.
    pub from: ObjectId,
    /// Where it must (or must not) reach.
    pub to: ObjectId,
    /// Required or forbidden.
    pub obligation: Obligation,
    /// What it may spend.
    /// What it may spend.
    ///
    /// ⚠ **A reference, not an owned limit.** *"Within carry range"* is a project-level concept; a
    /// route that owned its own `8.0` would drift from every other site the moment anyone retuned it.
    pub budget: BudgetRef,
    /// Must the whole path have line of sight?
    pub line_of_sight: bool,
    /// Must it be walkable from a standing start?
    pub from_standing: bool,
    /// How closely a party must stay together, 0..1. `1.0` means never separating.
    pub cohesion: f64,
    /// Content classes this route may not pass through.
    pub forbidden: Vec<ObjectId>,
}

impl Route {
    /// A route that must exist.
    pub fn required(from: ObjectId, to: ObjectId, budget: BudgetRef) -> Self {
        Route {
            from,
            to,
            obligation: Obligation::Required,
            budget,
            line_of_sight: false,
            from_standing: true,
            cohesion: 0.0,
            forbidden: Vec::new(),
        }
    }

    /// A route that must not exist.
    pub fn forbidden(from: ObjectId, to: ObjectId, budget: BudgetRef) -> Self {
        Route {
            obligation: Obligation::Forbidden,
            ..Route::required(from, to, budget)
        }
    }

    /// Require an unbroken sightline along the whole path.
    pub fn needing_line_of_sight(mut self) -> Self {
        self.line_of_sight = true;
        self
    }

    /// Refuse to pass through instances of these content classes.
    pub fn avoiding(mut self, kinds: impl IntoIterator<Item = ObjectId>) -> Self {
        self.forbidden.extend(kinds);
        self
    }

    /// Does a produced path satisfy this route?
    ///
    /// ⚠ **A forbidden route inverts the answer, not the reasoning.** A path that fits the budget
    /// *satisfies* a required route and *violates* a forbidden one, and having one function say so
    /// keeps the two from drifting apart.
    ///
    /// ⚠ Takes the project's [`BudgetBook`], because the route names a budget rather than owning one.
    /// A reference the book does not hold is `Unsuitable` — **not** `OverBudget`: no amount of moving
    /// the candidate fixes a budget that was never declared, so the search must stop rather than
    /// retry, and the message must name the missing budget.
    pub fn judge(&self, path: &Path, book: &BudgetBook) -> Verdict {
        let Some(budget) = self.budget.open(book) else {
            return Verdict::unsuitable(format!("no such budget: {}", self.budget));
        };
        let fits = budget.judge(path.length());
        match self.obligation {
            Obligation::Required => fits,
            Obligation::Forbidden => match fits {
                // The path exists and is affordable — which is exactly what must not happen.
                Verdict::Accepted { .. } => Verdict::Blocked { by: self.to },
                // It cannot be walked within budget, so the forbidden route does not exist.
                _ => Verdict::Accepted { slack: 0.0 },
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Serialization — verdicts and budgets reach the trace and the descriptor
// ---------------------------------------------------------------------------------------------

impl Serialize for Verdict {
    fn serialize(&self, w: &mut Writer) {
        match self {
            Verdict::Accepted { slack } => {
                w.u8(0);
                w.f64(*slack);
            }
            Verdict::OverBudget { excess, against } => {
                w.u8(1);
                w.f64(*excess);
                w.write(against);
            }
            Verdict::Blocked { by } => {
                w.u8(2);
                w.write(by);
            }
            Verdict::Unsuitable { reason } => {
                w.u8(3);
                w.str(reason);
            }
        }
    }
}

impl Deserialize for Verdict {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        match r.u8()? {
            0 => Ok(Verdict::Accepted { slack: r.f64()? }),
            1 => Ok(Verdict::OverBudget {
                excess: r.f64()?,
                against: r.read()?,
            }),
            2 => Ok(Verdict::Blocked {
                by: ObjectId::deserialize(r)?,
            }),
            3 => Ok(Verdict::Unsuitable {
                reason: r.str()?.to_string(),
            }),
            _ => Err(SerError::InvalidValue("unknown Verdict tag")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cv_determinism::math;

    fn oid(n: &str) -> ObjectId {
        ObjectId::derived("actor", n)
    }

    // --- the magnitude is the point ------------------------------------------------------------

    #[test]
    fn a_rejection_carries_the_number_the_solver_needs() {
        // ⚠ This is the whole reason `judge` does not return a bool: `OverBudget(6.2)` says *move it
        // 6.2 closer*, which converges. `false` says *try again*, which is a reroll.
        let v = Verdict::over_budget(6.2);
        assert_eq!(v.shortfall(), Some(6.2));
        assert!(v.is_retryable());
    }

    #[test]
    fn an_unsuitable_candidate_is_the_only_one_not_worth_retrying() {
        // ⚠ The search's termination condition. Without this distinction, "wrong kind of thing" is
        // indistinguishable from "wrong place" and the solver re-offers a doomed candidate forever.
        assert!(!Verdict::unsuitable("a door is not a floor").is_retryable());
        assert!(Verdict::blocked(oid("pillar")).is_retryable());
        assert!(Verdict::over_budget(1.0).is_retryable());
        assert!(Verdict::accepted(0.0).is_retryable());
    }

    #[test]
    fn over_budget_by_nothing_is_a_fit_not_a_rejection() {
        // A rejection the solver cannot act on is worse than no rejection.
        assert_eq!(Verdict::over_budget(0.0), Verdict::accepted(0.0));
        assert_eq!(Verdict::over_budget(-3.0), Verdict::accepted(3.0));
        assert!(Verdict::over_budget(-3.0).is_accepted());
    }

    #[test]
    fn slack_survives_into_the_verdict_rather_than_being_thresholded() {
        // ⚠ *Barely fits* and *fits easily* are different worlds, and difficulty reads the difference.
        match Verdict::accepted(0.05) {
            Verdict::Accepted { slack } => assert_eq!(slack, 0.05),
            other => panic!("expected acceptance, got {other}"),
        }
    }

    // --- budgets --------------------------------------------------------------------------------

    // ⚠ The budget suite lives in [`crate::budget`] now. What stays here is the one thing that is a
    // *judging* question rather than a budget question: that a verdict carries the budget's name.

    #[test]
    fn a_rejection_names_the_budget_it_was_measured_against() {
        // ⚠ *"Over budget by 6.2"* does not say against what. For a developer asking *"why did this
        // fail?"*, the name is the fact that turns a number into an action.
        let mut book = BudgetBook::new();
        let reach = book.declare("grapple reach", Cost::distance(30.0)).unwrap();
        let v = book.open(reach).unwrap().judge(36.2);

        assert_eq!(v.budget(), Some(reach));
        let excess = v.shortfall().expect("over budget carries a magnitude");
        // ⚠ Compared with a tolerance, not for equality: the excess is *computed*, and
        // `36.2 - 30.0` is `6.200000000000003`. Asserting exact equality on a derived float is the
        // habit the binding contract exists to break.
        assert!(
            math::abs(excess - 6.2) < 1e-9,
            "expected roughly 6.2, got {excess}"
        );
        assert!(v.to_string().contains("against"), "{v}");
    }

    #[test]
    fn a_fit_has_nothing_to_attribute_and_absorbs_the_name() {
        // ⚠ `against` chains rather than taking a constructor argument, so callers do not each write
        // the same `if` around a verdict that may or may not be a rejection.
        let fit = Verdict::accepted(4.0).against(oid("some budget"));
        assert_eq!(fit, Verdict::accepted(4.0));
        assert_eq!(fit.budget(), None);
    }

    // --- paths ----------------------------------------------------------------------------------

    #[test]
    fn a_path_that_doubles_back_is_longer_than_the_gap_it_crosses() {
        // ⚠ A budget is spent on travelling, not on displacement.
        let p = Path::from(Vec3::ZERO)
            .step_to(Vec3::new(10.0, 0.0, 0.0))
            .step_to(Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(p.length(), 20.0);
        assert_eq!(p.target(), Vec3::ZERO);
        assert_eq!(p.rise(), 0.0);
    }

    #[test]
    fn rise_is_signed_because_climbing_is_not_descending() {
        let up = Path::from(Vec3::ZERO).step_to(Vec3::new(0.0, 4.0, 0.0));
        let down = Path::from(Vec3::new(0.0, 4.0, 0.0)).step_to(Vec3::ZERO);
        assert_eq!(up.rise(), 4.0);
        assert_eq!(down.rise(), -4.0);
    }

    #[test]
    fn an_empty_path_ends_where_it_started() {
        let p = Path::from(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(p.target(), p.origin());
        assert_eq!(p.length(), 0.0);
    }

    // --- routes ---------------------------------------------------------------------------------

    /// A book with the one budget these routes name.
    fn routes_book() -> BudgetBook {
        let mut b = BudgetBook::new();
        b.declare("short hop", Cost::distance(30.0)).unwrap();
        b.declare("very short hop", Cost::distance(10.0)).unwrap();
        b
    }

    #[test]
    fn a_required_route_is_satisfied_by_a_path_within_budget() {
        let book = routes_book();
        let r = Route::required(oid("a"), oid("b"), BudgetRef::by_name("short hop"));
        let p = Path::from(Vec3::ZERO).step_to(Vec3::new(20.0, 0.0, 0.0));
        assert!(r.judge(&p, &book).is_accepted());
    }

    #[test]
    fn a_route_naming_a_budget_that_does_not_exist_stops_the_search() {
        // ⚠ **`Unsuitable`, not `OverBudget`.** No amount of moving the candidate fixes a budget that
        // was never declared, so the search must terminate rather than retry — and the message has to
        // name what is missing, or the developer is left guessing.
        let book = routes_book();
        let r = Route::required(oid("a"), oid("b"), BudgetRef::by_name("typo'd name"));
        let p = Path::from(Vec3::ZERO).step_to(Vec3::new(1.0, 0.0, 0.0));
        let v = r.judge(&p, &book);
        assert!(!v.is_retryable());
        assert!(v.to_string().contains("no such budget"), "{v}");
    }

    #[test]
    fn an_inline_cost_needs_no_book_entry() {
        // ⚠ The one-off case must not require registering a name first, or every throwaway route pays
        // ceremony for a number used once.
        let book = BudgetBook::new();
        let r = Route::required(oid("a"), oid("b"), BudgetRef::distance(30.0));
        let p = Path::from(Vec3::ZERO).step_to(Vec3::new(20.0, 0.0, 0.0));
        assert!(r.judge(&p, &book).is_accepted());
    }

    #[test]
    fn a_forbidden_route_inverts_the_answer_and_not_the_reasoning() {
        // ⚠ *"The boss must not be reachable from the entrance without the key"* is the same kind of
        // statement as *"these two must connect"*, and one search satisfies both.
        let walkable = Path::from(Vec3::ZERO).step_to(Vec3::new(20.0, 0.0, 0.0));
        let far = Path::from(Vec3::ZERO).step_to(Vec3::new(500.0, 0.0, 0.0));
        let book = routes_book();
        let r = Route::forbidden(
            oid("entrance"),
            oid("boss"),
            BudgetRef::by_name("short hop"),
        );

        assert!(
            !r.judge(&walkable, &book).is_accepted(),
            "an affordable path is exactly what a forbidden route must not have"
        );
        assert!(
            r.judge(&far, &book).is_accepted(),
            "unaffordable means the forbidden route does not exist, which satisfies it"
        );
    }

    #[test]
    fn route_predicates_are_declared_not_inferred() {
        let r = Route::required(oid("a"), oid("b"), BudgetRef::by_name("very short hop"))
            .needing_line_of_sight()
            .avoiding([oid("lava")]);
        assert!(r.line_of_sight);
        assert!(r.from_standing, "walkable from a standing start by default");
        assert_eq!(r.forbidden, vec![oid("lava")]);
    }

    // --- the wire form ----------------------------------------------------------------------------

    #[test]
    fn verdicts_round_trip_with_their_magnitudes_intact() {
        use crate::serialize::{from_bytes, to_bytes};
        for v in [
            Verdict::accepted(1.5),
            Verdict::over_budget(6.2),
            Verdict::blocked(oid("pillar")),
            Verdict::unsuitable("wrong kind"),
        ] {
            assert_eq!(from_bytes::<Verdict>(&to_bytes(&v)).unwrap(), v);
        }
    }
}
