//! **Budgets** — named, retunable limits, and the accounting against them.
//!
//! # The problem this shape exists to solve
//!
//! ⚠ **A magic number in twelve places is twelve places to miss one.** *"Carry range"* is not a number,
//! it is a **concept a project tunes**; it appears on a gate rule, on a route, on three components and
//! in a preset, and a developer who retunes it by editing `8.0` at each site has already lost. Naming
//! it once and pointing at it is the whole feature.
//!
//! ⚠ **And a verdict that says `over budget by 6.2` does not say *against what*.** For a developer
//! asking *"why did this placement fail?"* the budget's **name** is the single most useful missing
//! fact, and it is free once budgets have names.
//!
//! # Declaration and accounting are two things
//!
//! | Type | Is | Lives |
//! |---|---|---|
//! | [`Cost`] | *"at most 8 metres"* — a kind and a limit | authored, immutable |
//! | [`Budget`] | a cost **being spent against**, with a name and an id | a working copy per evaluation |
//! | [`BudgetBook`] | the project's named budgets | one per project, retuned in one place |
//! | [`BudgetRef`] | *"this budget"* — named, or inline | on rules, routes and components |
//!
//! ⚠ **The book holds unspent budgets and nothing spends against the book's copy.** A solve
//! [`BudgetBook::open`]s a working copy; spending against the shared row would make two unrelated
//! routes drain each other, and the bug would look like *"placement gets worse the longer generation
//! runs"*, which is close to untraceable.
//!
//! # Why both named and inline
//!
//! ⚠ **Forcing every one-off through the book is friction, and forbidding names is worse.** A jump
//! that is 4 metres once wants to say `4 metres` where it is written; a *carry range* wants a name. So
//! [`BudgetRef`] carries either — and because the two are distinguishable, the editor can offer
//! *"this inline cost appears in 12 places — extract it?"*, which is the refactor that stops the magic
//! number from spreading in the first place.

use crate::class::CoreClass;
use crate::object::{Object, ObjectHeader, ObjectId};
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use std::collections::BTreeMap;
use std::fmt;

/// `/Core/Budget`, so a budget can be named by a [`crate::Ref`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BudgetClass;

impl CoreClass for BudgetClass {
    const PATH: &'static str = "/Core/Budget";
}

/// **What something costs** — a kind and a limit, with no accounting.
///
/// ⚠ **Every `Time` cost is a distance divided by `player_profile.speed`**, which is why that setting
/// is not optional: without a speed there is no way to turn seconds into a reachable distance, and the
/// whole time axis becomes unusable.
#[derive(Clone, Debug, PartialEq)]
pub enum Cost {
    /// World units.
    Distance { limit: f64 },
    /// Seconds, converted through the player's speed.
    Time { limit: f64, speed: f64 },
    /// A draw against a named resource pool, at a rate per world unit.
    ///
    /// ⚠ **This is how a soft gate becomes a magnitude rather than a rule.** *"You can cross the lava
    /// if you have enough hearts"* is a budget question, not a lock — and expressing it as one means
    /// the solver can trade it off instead of treating it as impassable.
    Pool { pool: String, limit: f64, rate: f64 },
}

impl Cost {
    /// A distance limit in world units.
    pub fn distance(limit: f64) -> Self {
        Cost::Distance { limit }
    }

    /// A time limit in seconds, travelled at `speed` world units per second.
    pub fn time(limit: f64, speed: f64) -> Self {
        Cost::Time { limit, speed }
    }

    /// A draw against a named pool.
    pub fn pool(pool: impl Into<String>, limit: f64, rate: f64) -> Self {
        Cost::Pool {
            pool: pool.into(),
            limit,
            rate,
        }
    }

    /// Nothing — a free move.
    pub fn free() -> Self {
        Cost::Distance { limit: 0.0 }
    }

    /// The limit, in whatever this cost measures.
    pub fn limit(&self) -> f64 {
        match self {
            Cost::Distance { limit } | Cost::Time { limit, .. } | Cost::Pool { limit, .. } => {
                *limit
            }
        }
    }

    /// What `distance` world units draw from this cost.
    ///
    /// ⚠ **The argument is always a distance**, whatever the cost measures. A caller that had to know
    /// whether to pass metres or seconds would be re-deriving the conversion at every call site, and
    /// one of them would get it wrong.
    pub fn draw(&self, distance: f64) -> f64 {
        match self {
            Cost::Distance { .. } => distance,
            Cost::Time { speed, .. } => {
                if *speed > 0.0 {
                    distance / *speed
                } else {
                    0.0
                }
            }
            Cost::Pool { rate, .. } => distance * *rate,
        }
    }

    /// The unit this is measured in, for a trace a developer reads.
    pub fn unit(&self) -> &str {
        match self {
            Cost::Distance { .. } => "m",
            Cost::Time { .. } => "s",
            Cost::Pool { pool, .. } => pool,
        }
    }
}

impl fmt::Display for Cost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.limit(), self.unit())
    }
}

/// A cost **being spent against**, with an identity so a verdict can name it.
///
/// ⚠ **Identity is the point.** Without it `over budget by 6.2` is the whole diagnosis; with it the
/// trace reads *"over budget by 6.2 m against grapple reach"*, which is the difference between a
/// developer knowing what to change and a developer guessing.
#[derive(Clone, Debug, PartialEq)]
pub struct Budget {
    header: ObjectHeader,
    cost: Cost,
    spent: f64,
}

impl Budget {
    /// A named budget, unspent.
    pub fn named(name: impl Into<String>, cost: Cost) -> Self {
        let name = name.into();
        Budget {
            header: ObjectHeader::derived("budget", name),
            cost,
            spent: 0.0,
        }
    }

    /// An **anonymous** budget, for a cost authored where it is used.
    ///
    /// ⚠ Its id is derived from the cost, not allocated, so two identical inline costs share an id —
    /// which is what lets the editor notice that the same magic number appears in twelve places.
    pub fn anonymous(cost: Cost) -> Self {
        let key = format!("{cost:?}");
        Budget {
            header: ObjectHeader::new(ObjectId::derived("budget_inline", &key), cost.to_string()),
            cost,
            spent: 0.0,
        }
    }

    /// What it costs.
    pub fn cost(&self) -> &Cost {
        &self.cost
    }

    /// How much has been drawn.
    pub fn spent(&self) -> f64 {
        self.spent
    }

    /// What is left.
    pub fn remaining(&self) -> f64 {
        self.cost.limit() - self.spent
    }

    /// Spend `distance` world units against it.
    pub fn spend(&mut self, distance: f64) {
        self.spent += self.cost.draw(distance);
    }

    /// Has it been drawn from at all?
    ///
    /// ⚠ The predicate that says whether this is a **declaration** or a working copy. Nothing in a
    /// [`BudgetBook`] should ever answer `true`.
    pub fn is_spent_against(&self) -> bool {
        self.spent != 0.0
    }

    /// The same budget with its limit **scaled**, for supply pressure.
    ///
    /// ⚠ **The consumer of a `consumption_pressure` dial.** *"How hard resources are squeezed against
    /// supply"* is a scale on what counts as affordable: a factor **below 1 tightens** every budget it
    /// touches, above 1 loosens. Applied to a *working copy*, never to the book's row — pressure is a
    /// property of a generation pass, not of the project's tuning.
    ///
    /// ⚠ **The core reads no dial here.** It offers the scaling; a *developer's* graph supplies the
    /// factor from whatever they named it. A core-shipped `consumption_pressure` would be exactly the
    /// core dial the design says does not exist.
    pub fn under_pressure(mut self, factor: f64) -> Self {
        let f = if factor.is_finite() && factor > 0.0 {
            factor
        } else {
            1.0
        };
        self.cost = match &self.cost {
            Cost::Distance { limit } => Cost::Distance { limit: limit * f },
            Cost::Time { limit, speed } => Cost::Time {
                limit: limit * f,
                speed: *speed,
            },
            Cost::Pool { pool, limit, rate } => Cost::Pool {
                pool: pool.clone(),
                limit: limit * f,
                rate: *rate,
            },
        };
        self
    }

    /// Judge a distance against what is left, **naming this budget in the verdict**.
    ///
    /// The bridge from budgets to [`crate::Verdict`], and the reason a route rejection carries both a
    /// number the solver can act on and a name a developer can act on.
    pub fn judge(&self, distance: f64) -> crate::judge::Verdict {
        let excess = self.cost.draw(distance) - self.remaining();
        crate::judge::Verdict::over_budget(excess).against(self.id())
    }
}

impl Object for Budget {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut ObjectHeader {
        &mut self.header
    }
    fn type_name(&self) -> &'static str {
        "Budget"
    }
}

impl fmt::Display for Budget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name(), self.cost)
    }
}

/// **This budget** — named, or authored right here.
///
/// ⚠ The two are kept distinguishable rather than normalised to one, because that is what lets a tool
/// say *"this inline cost appears in twelve places — extract it?"*. Collapsing them would throw away
/// the only signal that the magic number is spreading.
#[derive(Clone, Debug, PartialEq)]
pub enum BudgetRef {
    /// A row of the project's [`BudgetBook`] — retune it in one place.
    Named(ObjectId),
    /// A cost authored at this site. Fine for a one-off.
    Inline(Cost),
}

impl BudgetRef {
    /// Point at a named budget.
    pub fn named(id: ObjectId) -> Self {
        BudgetRef::Named(id)
    }

    /// Point at a named budget by the name it was registered under.
    pub fn by_name(name: &str) -> Self {
        BudgetRef::Named(ObjectId::derived("budget", name))
    }

    /// Author a distance here.
    pub fn distance(limit: f64) -> Self {
        BudgetRef::Inline(Cost::distance(limit))
    }

    /// Author a time here.
    pub fn time(limit: f64, speed: f64) -> Self {
        BudgetRef::Inline(Cost::time(limit, speed))
    }

    /// Author a pool draw here.
    pub fn pool(pool: impl Into<String>, limit: f64, rate: f64) -> Self {
        BudgetRef::Inline(Cost::pool(pool, limit, rate))
    }

    /// Nothing — a free move.
    pub fn free() -> Self {
        BudgetRef::Inline(Cost::free())
    }

    /// The named budget's id, if this is a reference.
    pub fn id(&self) -> Option<ObjectId> {
        match self {
            BudgetRef::Named(id) => Some(*id),
            BudgetRef::Inline(_) => None,
        }
    }

    /// Open a working copy to spend against.
    ///
    /// ⚠ Returns `None` for a `Named` reference the book does not hold — a **dangling budget
    /// reference**, which is a load-time diagnostic and not something to paper over with a default.
    /// Silently substituting a limit would produce a world that generates and is wrong.
    pub fn open(&self, book: &BudgetBook) -> Option<Budget> {
        match self {
            BudgetRef::Named(id) => book.open(*id),
            BudgetRef::Inline(cost) => Some(Budget::anonymous(cost.clone())),
        }
    }
}

impl fmt::Display for BudgetRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BudgetRef::Named(id) => write!(f, "budget {id}"),
            BudgetRef::Inline(cost) => write!(f, "{cost}"),
        }
    }
}

/// What can go wrong with the book.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BudgetError {
    /// Two budgets registered under one name.
    Duplicate { name: String },
    /// A reference names a budget the book does not hold.
    Dangling { id: ObjectId },
}

impl fmt::Display for BudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BudgetError::Duplicate { name } => write!(f, "two budgets named {name:?}"),
            BudgetError::Dangling { id } => write!(f, "no budget {id} is registered"),
        }
    }
}

impl std::error::Error for BudgetError {}

/// **The project's named budgets** — the one place *"carry range"* is a number.
///
/// ⚠ **Nothing spends against a row here.** [`Self::open`] hands out a working copy; spending against
/// the shared row would make two unrelated routes drain each other, and the symptom — *"placement gets
/// worse the longer generation runs"* — points nowhere near the cause.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BudgetBook {
    budgets: BTreeMap<ObjectId, Budget>,
}

impl BudgetBook {
    /// An empty book.
    pub fn new() -> Self {
        BudgetBook::default()
    }

    /// Register a named budget and return its id.
    pub fn declare(&mut self, name: &str, cost: Cost) -> Result<ObjectId, BudgetError> {
        let budget = Budget::named(name, cost);
        let id = budget.id();
        if self.budgets.contains_key(&id) {
            return Err(BudgetError::Duplicate {
                name: name.to_string(),
            });
        }
        self.budgets.insert(id, budget);
        Ok(id)
    }

    /// **Retune a budget** — the whole reason budgets have names.
    ///
    /// ⚠ Every site pointing at it changes at once, which is the point; a site that had inlined the
    /// number does not, which is also the point.
    pub fn retune(&mut self, id: ObjectId, cost: Cost) -> Result<(), BudgetError> {
        let budget = self
            .budgets
            .get_mut(&id)
            .ok_or(BudgetError::Dangling { id })?;
        budget.cost = cost;
        budget.spent = 0.0;
        Ok(())
    }

    /// The declaration, unspent.
    pub fn get(&self, id: ObjectId) -> Option<&Budget> {
        self.budgets.get(&id)
    }

    /// Look one up by the name it was declared under.
    pub fn by_name(&self, name: &str) -> Option<&Budget> {
        self.budgets.get(&ObjectId::derived("budget", name))
    }

    /// A **working copy** to spend against.
    pub fn open(&self, id: ObjectId) -> Option<Budget> {
        self.budgets.get(&id).cloned()
    }

    /// How many budgets are declared.
    pub fn len(&self) -> usize {
        self.budgets.len()
    }

    /// Is the book empty?
    pub fn is_empty(&self) -> bool {
        self.budgets.is_empty()
    }

    /// Every declared budget, in id order.
    pub fn iter(&self) -> impl Iterator<Item = &Budget> {
        self.budgets.values()
    }

    /// Check that every reference resolves.
    ///
    /// ⚠ **A load-time sweep, not a runtime fallback.** A dangling budget reference must fail loudly
    /// before generation, because the alternative — a default limit quietly standing in — produces a
    /// world that builds and is wrong.
    pub fn check<'a>(
        &self,
        refs: impl IntoIterator<Item = &'a BudgetRef>,
    ) -> Result<(), BudgetError> {
        for r in refs {
            if let BudgetRef::Named(id) = r {
                if !self.budgets.contains_key(id) {
                    return Err(BudgetError::Dangling { id: *id });
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Wire form
// ---------------------------------------------------------------------------------------------

impl Serialize for Cost {
    fn serialize(&self, w: &mut Writer) {
        match self {
            Cost::Distance { limit } => {
                w.u8(0);
                w.f64(*limit);
            }
            Cost::Time { limit, speed } => {
                w.u8(1);
                w.f64(*limit);
                w.f64(*speed);
            }
            Cost::Pool { pool, limit, rate } => {
                w.u8(2);
                w.str(pool);
                w.f64(*limit);
                w.f64(*rate);
            }
        }
    }
}

impl Deserialize for Cost {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(match r.u8()? {
            0 => Cost::Distance { limit: r.f64()? },
            1 => Cost::Time {
                limit: r.f64()?,
                speed: r.f64()?,
            },
            2 => Cost::Pool {
                pool: r.str()?,
                limit: r.f64()?,
                rate: r.f64()?,
            },
            _ => return Err(SerError::InvalidValue("unknown Cost tag")),
        })
    }
}

impl Serialize for BudgetRef {
    /// ⚠ **`spent` is never written**, because a `BudgetRef` is a declaration. Persisting accounting
    /// state would make two loads of the same file behave differently.
    fn serialize(&self, w: &mut Writer) {
        match self {
            BudgetRef::Named(id) => {
                w.u8(0);
                w.write(id);
            }
            BudgetRef::Inline(cost) => {
                w.u8(1);
                w.write(cost);
            }
        }
    }
}

impl Deserialize for BudgetRef {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(match r.u8()? {
            0 => BudgetRef::Named(r.read()?),
            1 => BudgetRef::Inline(r.read()?),
            _ => return Err(SerError::InvalidValue("unknown BudgetRef tag")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> (BudgetBook, ObjectId) {
        let mut b = BudgetBook::new();
        let carry = b.declare("carry range", Cost::distance(8.0)).unwrap();
        b.declare("grapple reach", Cost::distance(30.0)).unwrap();
        b.declare("air supply", Cost::time(60.0, 5.0)).unwrap();
        (b, carry)
    }

    // --- the QoL claims, as tests ------------------------------------------------------------

    #[test]
    fn retuning_one_row_moves_every_site_that_named_it() {
        // ⚠ **The whole reason budgets have names.** A project that inlined `8.0` in twelve places
        // retunes twelve places and misses one; a project that named it retunes one.
        let (mut b, carry) = book();
        let gate = BudgetRef::named(carry);
        let route = BudgetRef::named(carry);

        assert_eq!(gate.open(&b).unwrap().remaining(), 8.0);
        assert_eq!(route.open(&b).unwrap().remaining(), 8.0);

        b.retune(carry, Cost::distance(12.0)).unwrap();
        assert_eq!(gate.open(&b).unwrap().remaining(), 12.0);
        assert_eq!(route.open(&b).unwrap().remaining(), 12.0);
    }

    #[test]
    fn an_inline_cost_does_not_move_when_a_named_one_is_retuned() {
        // The other half: inlining is a *choice*, and its consequence is exactly this.
        let (mut b, carry) = book();
        let inlined = BudgetRef::distance(8.0);
        b.retune(carry, Cost::distance(12.0)).unwrap();
        assert_eq!(inlined.open(&b).unwrap().remaining(), 8.0);
    }

    #[test]
    fn identical_inline_costs_share_an_id_so_a_tool_can_spot_the_duplication() {
        // ⚠ The signal that lets an editor offer *"this appears in twelve places — extract it?"*.
        // Allocating ids would destroy it, and normalising inline into named would destroy the
        // distinction entirely.
        let a = Budget::anonymous(Cost::distance(8.0));
        let c = Budget::anonymous(Cost::distance(8.0));
        let d = Budget::anonymous(Cost::distance(9.0));
        assert_eq!(a.id(), c.id());
        assert_ne!(a.id(), d.id());
    }

    #[test]
    fn a_verdict_names_the_budget_it_was_measured_against() {
        // ⚠ *"over budget by 6.2"* does not say against what. For *"why did this fail?"* the name is
        // the single most useful missing fact, and it is free once budgets have names.
        let (b, carry) = book();
        let budget = b.open(carry).unwrap();
        let v = budget.judge(14.2);
        assert!(!v.is_accepted());
        assert_eq!(v.budget(), Some(carry));
        assert!(v.shortfall().is_some_and(|s| (s - 6.2).abs() < 1e-9), "{v}");
    }

    // --- declaration versus accounting ---------------------------------------------------------

    #[test]
    fn nothing_spends_against_the_book() {
        // ⚠ Spending against the shared row would make two unrelated routes drain each other, and the
        // symptom — *"placement gets worse the longer generation runs"* — points nowhere near the
        // cause.
        let (b, carry) = book();
        let mut first = b.open(carry).unwrap();
        first.spend(5.0);
        assert_eq!(first.remaining(), 3.0);

        let second = b.open(carry).unwrap();
        assert_eq!(second.remaining(), 8.0, "the book is untouched");
        assert!(!b.get(carry).unwrap().is_spent_against());
    }

    #[test]
    fn a_declaration_is_distinguishable_from_a_working_copy() {
        let (b, carry) = book();
        let mut working = b.open(carry).unwrap();
        assert!(!working.is_spent_against());
        working.spend(1.0);
        assert!(working.is_spent_against());
    }

    // --- the costs themselves --------------------------------------------------------------------

    #[test]
    fn every_cost_is_spent_in_world_units_whatever_it_measures() {
        // ⚠ A caller that had to know whether to pass metres or seconds would re-derive the conversion
        // at every call site, and one of them would get it wrong.
        assert_eq!(Cost::distance(10.0).draw(4.0), 4.0);
        assert_eq!(Cost::time(10.0, 2.0).draw(4.0), 2.0, "4 m at 2 m/s is 2 s");
        assert_eq!(Cost::pool("hearts", 3.0, 0.25).draw(4.0), 1.0);
    }

    #[test]
    fn a_zero_speed_time_cost_draws_nothing_rather_than_dividing_by_zero() {
        // A misconfigured profile must degrade to *"free"*, not to `inf` or `NaN` — a poisoned number
        // would spread through every comparison downstream.
        let c = Cost::time(10.0, 0.0);
        assert_eq!(c.draw(100.0), 0.0);
        assert!(c.draw(100.0).is_finite());
    }

    #[test]
    fn a_cost_says_what_it_is_measured_in() {
        assert_eq!(Cost::distance(8.0).unit(), "m");
        assert_eq!(Cost::time(8.0, 1.0).unit(), "s");
        assert_eq!(Cost::pool("hearts", 3.0, 1.0).unit(), "hearts");
        assert_eq!(Cost::pool("hearts", 3.0, 1.0).to_string(), "3 hearts");
    }

    // --- the book's own rules ----------------------------------------------------------------

    #[test]
    fn a_dangling_reference_fails_loudly_rather_than_defaulting() {
        // ⚠ A default limit quietly standing in produces a world that builds and is wrong — the worst
        // possible outcome, because nothing points at the cause.
        let (b, _) = book();
        let ghost = BudgetRef::by_name("no such budget");
        assert_eq!(ghost.open(&b), None);
        assert!(matches!(
            b.check([&ghost]),
            Err(BudgetError::Dangling { .. })
        ));
    }

    #[test]
    fn the_load_time_sweep_passes_inline_costs_and_resolves_named_ones() {
        let (b, carry) = book();
        let refs = [
            BudgetRef::named(carry),
            BudgetRef::distance(4.0),
            BudgetRef::by_name("grapple reach"),
        ];
        assert!(b.check(refs.iter()).is_ok());
    }

    #[test]
    fn a_name_is_the_key_so_two_budgets_cannot_share_one() {
        let (mut b, _) = book();
        assert!(matches!(
            b.declare("carry range", Cost::distance(99.0)),
            Err(BudgetError::Duplicate { .. })
        ));
        assert_eq!(b.len(), 3);
    }

    #[test]
    fn a_budget_is_found_by_the_name_a_developer_typed() {
        let (b, _) = book();
        assert_eq!(b.by_name("grapple reach").unwrap().remaining(), 30.0);
        assert_eq!(b.by_name("Grapple Reach"), None, "names are exact");
    }

    #[test]
    fn retuning_something_that_is_not_there_is_refused() {
        let (mut b, _) = book();
        assert!(matches!(
            b.retune(ObjectId::derived("budget", "ghost"), Cost::distance(1.0)),
            Err(BudgetError::Dangling { .. })
        ));
    }

    // --- wire ---------------------------------------------------------------------------------

    #[test]
    fn a_reference_round_trips_and_carries_no_accounting() {
        // ⚠ Persisting `spent` would make two loads of the same file behave differently.
        use crate::serialize::{from_bytes, to_bytes};
        for r in [
            BudgetRef::by_name("carry range"),
            BudgetRef::distance(8.0),
            BudgetRef::time(60.0, 5.0),
            BudgetRef::pool("hearts", 3.0, 0.5),
        ] {
            assert_eq!(from_bytes::<BudgetRef>(&to_bytes(&r)).unwrap(), r);
        }
    }

    #[test]
    fn pressure_tightens_what_counts_as_affordable() {
        // ⚠ The consumer a `consumption_pressure` dial reaches. A factor below 1 squeezes supply, so a
        // route that fitted comfortably stops fitting — which is the point of the dial.
        let (b, carry) = book();
        let normal = b.open(carry).unwrap();
        assert!(normal.judge(7.0).is_accepted());

        let squeezed = b.open(carry).unwrap().under_pressure(0.5);
        assert_eq!(squeezed.remaining(), 4.0);
        assert!(!squeezed.judge(7.0).is_accepted());
        assert_eq!(
            squeezed.judge(7.0).budget(),
            Some(carry),
            "and it is still the same budget, so the trace still names it"
        );
    }

    #[test]
    fn pressure_applies_to_a_working_copy_and_never_to_the_book() {
        // ⚠ Pressure is a property of a *generation pass*, not of the project's tuning. Writing it
        // back would make a project's authored limits drift every time a pass ran.
        let (b, carry) = book();
        let _ = b.open(carry).unwrap().under_pressure(0.25);
        assert_eq!(b.get(carry).unwrap().remaining(), 8.0);
    }

    #[test]
    fn a_nonsense_pressure_factor_leaves_the_budget_alone() {
        // Zero, negative and NaN would each poison every comparison downstream. Degrading to
        // *"unchanged"* is the only safe reading of a factor that cannot mean anything.
        let (b, carry) = book();
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                b.open(carry).unwrap().under_pressure(bad).remaining(),
                8.0,
                "factor {bad}"
            );
        }
    }

    #[test]
    fn a_budget_describes_itself_for_a_trace_a_developer_reads() {
        let (b, carry) = book();
        assert_eq!(b.get(carry).unwrap().to_string(), "carry range (8 m)");
        assert_eq!(
            Budget::anonymous(Cost::pool("hearts", 3.0, 1.0)).to_string(),
            "3 hearts (3 hearts)"
        );
    }
}
