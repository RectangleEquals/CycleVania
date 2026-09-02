//! **Budgets as things a project tunes**, not numbers a developer retypes.
//!
//! This closes the delta M08 left open: three fields the design types `Ref<Budget>` were owning a
//! `Budget` value. The reference machinery was never the blocker — `Budget`'s own shape was, because it
//! fused the *declaration* with the *accounting*.
//!
//! # The two failures it was causing
//!
//! ⚠ **A magic number in twelve places is twelve places to miss one.** *"Carry range"* is a concept a
//! project tunes; it appears on a gate rule, a route, several components and a preset. A developer
//! retuning it by editing `8.0` at each site has already lost, and the one they miss will be the one
//! that matters.
//!
//! ⚠ **`over budget by 6.2` does not say against what.** For *"why did this placement fail?"* the
//! budget's name is the fact that turns a number into an action — and it costs nothing once budgets
//! have names.
//!
//! # What a developer actually writes
//!
//! Both forms stay available, deliberately. A jump that is four metres once says `4 metres` where it is
//! written; a carry range gets a name. Because the two are distinguishable, a tool can notice the
//! second case and offer to extract it — which is what stops the magic number spreading in the first
//! place.

use cv_core::budget::{Budget, BudgetBook, BudgetError, BudgetRef, Cost};
use cv_core::class::{BudgetBound, CoreClass};
use cv_core::component::{Attached, Component, Components};
use cv_core::judge::{Path, Route, Verdict};
use cv_core::mission::Rule;
use cv_core::surface::Support;
use cv_core::{ClassPath, ClassRegistry, InstanceScope, Kind, Object, ObjectId};
use cv_determinism::Vec3;

fn class(p: &str) -> ClassPath {
    ClassPath::new(p).unwrap()
}
fn oid(n: &str) -> ObjectId {
    ObjectId::derived("actor", n)
}

/// A project's budgets: the numbers it tunes, named once.
fn book() -> BudgetBook {
    let mut b = BudgetBook::new();
    b.declare("carry range", Cost::distance(8.0)).unwrap();
    b.declare("grapple reach", Cost::distance(30.0)).unwrap();
    b.declare("air supply", Cost::time(90.0, 5.0)).unwrap();
    b.declare("lava crossing", Cost::pool("hearts", 3.0, 0.25))
        .unwrap();
    b
}

// ---------------------------------------------------------------------------------------------
// Retuning in one place
// ---------------------------------------------------------------------------------------------

#[test]
fn retuning_carry_range_moves_every_site_that_named_it() {
    // ⚠ **The whole point.** Five unrelated authoring sites, one edit — and nothing was told about the
    // others, because nothing needed to be.
    let mut b = book();
    let carry = BudgetRef::by_name("carry range");

    let gate = Rule::Nearby {
        kind: class("/Content/Props/BombFlower"),
        within: carry.clone(),
        scope: InstanceScope::Space,
    };
    let route = Route::required(oid("lever"), oid("door"), carry.clone());
    let ledge = Support::always(40.0).lasting(carry.clone());
    let network = Component::FastTravel {
        network: "stagways".into(),
        cost: Some(carry.clone()),
        unlocked_by: Rule::Always,
    };

    let reach_of = |r: &BudgetRef, b: &BudgetBook| r.open(b).unwrap().remaining();
    let Rule::Nearby { within, .. } = &gate else {
        panic!("a nearby rule")
    };
    let Component::FastTravel { cost, .. } = &network else {
        panic!("a network")
    };

    for r in [
        within,
        &route.budget,
        ledge.endurance.as_ref().unwrap(),
        cost.as_ref().unwrap(),
    ] {
        assert_eq!(reach_of(r, &b), 8.0);
    }

    // One edit.
    b.retune(carry.id().unwrap(), Cost::distance(12.0)).unwrap();

    for r in [
        within,
        &route.budget,
        ledge.endurance.as_ref().unwrap(),
        cost.as_ref().unwrap(),
    ] {
        assert_eq!(reach_of(r, &b), 12.0, "every site moved, and none was told");
    }
}

#[test]
fn inlining_is_a_choice_and_this_is_what_it_costs() {
    // ⚠ Not a trap — a deliberate trade. A one-off number stays where it was written, which is right
    // for a one-off and wrong for a concept, and the type says which one a developer picked.
    let mut b = book();
    let inlined = BudgetRef::distance(8.0);
    b.retune(
        BudgetRef::by_name("carry range").id().unwrap(),
        Cost::distance(12.0),
    )
    .unwrap();
    assert_eq!(inlined.open(&b).unwrap().remaining(), 8.0);
}

#[test]
fn the_same_inline_number_in_many_places_is_a_visible_signal() {
    // ⚠ Identical inline costs share a derived id, so a tool can say *"this appears in four places —
    // extract it?"*. Allocating ids, or normalising inline into named, would both destroy the signal.
    let sites = [
        BudgetRef::distance(8.0),
        BudgetRef::distance(8.0),
        BudgetRef::distance(8.0),
        BudgetRef::distance(30.0),
    ];
    let b = BudgetBook::new();
    let ids: Vec<ObjectId> = sites.iter().map(|s| s.open(&b).unwrap().id()).collect();
    assert_eq!(ids[0], ids[1]);
    assert_eq!(ids[1], ids[2]);
    assert_ne!(ids[2], ids[3]);
    assert_eq!(
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        2,
        "four sites, two distinct numbers"
    );
}

// ---------------------------------------------------------------------------------------------
// The name in the verdict
// ---------------------------------------------------------------------------------------------

#[test]
fn a_rejection_says_which_lever_to_pull() {
    // ⚠ *"Over budget by 6.2"* leaves a developer guessing. *"…against grapple reach"* does not, and
    // the difference is one field that costs nothing to carry.
    let b = book();
    let reach = BudgetRef::by_name("grapple reach");
    let v = reach.open(&b).unwrap().judge(36.2);

    assert_eq!(v.budget(), reach.id());
    assert_eq!(
        b.get(v.budget().unwrap()).unwrap().name(),
        "grapple reach",
        "and the id resolves back to the name a developer typed"
    );
    assert!(v.shortfall().is_some_and(|s| (s - 6.2).abs() < 1e-9), "{v}");
}

#[test]
fn a_route_rejection_carries_the_name_through_to_the_trace() {
    let b = book();
    let r = Route::required(
        oid("entrance"),
        oid("vault"),
        BudgetRef::by_name("carry range"),
    );
    let far = Path::from(Vec3::ZERO).step_to(Vec3::new(50.0, 0.0, 0.0));
    let v = r.judge(&far, &b);

    assert!(!v.is_accepted());
    assert_eq!(v.budget(), BudgetRef::by_name("carry range").id());
    assert!(v.to_string().contains("against"), "{v}");
}

#[test]
fn a_fit_attributes_nothing_because_there_is_nothing_to_attribute() {
    let b = book();
    let r = Route::required(oid("a"), oid("b"), BudgetRef::by_name("grapple reach"));
    let near = Path::from(Vec3::ZERO).step_to(Vec3::new(10.0, 0.0, 0.0));
    let v = r.judge(&near, &b);
    assert!(v.is_accepted());
    assert_eq!(v.budget(), None);
}

// ---------------------------------------------------------------------------------------------
// Declaration versus accounting
// ---------------------------------------------------------------------------------------------

#[test]
fn two_routes_naming_one_budget_do_not_drain_each_other() {
    // ⚠ **The bug the split exists to prevent.** Spending against the shared row would make placement
    // get quietly worse the longer generation ran, and the symptom points nowhere near the cause.
    let b = book();
    let air = BudgetRef::by_name("air supply");

    let mut first = air.open(&b).unwrap();
    first.spend(400.0);
    assert!(first.is_spent_against());

    let second = air.open(&b).unwrap();
    assert!(!second.is_spent_against());
    assert_eq!(second.remaining(), 90.0);
    assert!(!b.get(air.id().unwrap()).unwrap().is_spent_against());
}

#[test]
fn a_pool_budget_still_reads_as_a_magnitude_the_solver_can_trade() {
    // ⚠ *"You can cross the lava if you have enough hearts"* is a budget question, not a lock — which
    // is what lets the solver trade it off instead of treating the lava as impassable.
    let b = book();
    let lava = BudgetRef::by_name("lava crossing").open(&b).unwrap();
    assert_eq!(lava.cost().unit(), "hearts");

    // 12 metres of lava at a quarter-heart per metre is three hearts — exactly the limit.
    assert!(lava.judge(12.0).is_accepted());
    // A metre further is a magnitude, not a refusal.
    let over = lava.judge(16.0);
    assert!(over.shortfall().is_some_and(|s| s > 0.0), "{over}");
    assert_eq!(over.budget(), Some(lava.id()));
}

// ---------------------------------------------------------------------------------------------
// Failing loudly
// ---------------------------------------------------------------------------------------------

#[test]
fn a_typo_in_a_budget_name_stops_the_search_instead_of_defaulting() {
    // ⚠ **`Unsuitable`, not `OverBudget`.** No amount of moving the candidate fixes a budget that was
    // never declared. A default limit standing in would produce a world that generates and is wrong,
    // which is the worst outcome because nothing points at the cause.
    let b = book();
    let r = Route::required(oid("a"), oid("b"), BudgetRef::by_name("carry rage"));
    let p = Path::from(Vec3::ZERO).step_to(Vec3::new(1.0, 0.0, 0.0));
    let v = r.judge(&p, &b);

    assert!(!v.is_retryable(), "a search must not retry this");
    assert!(matches!(v, Verdict::Unsuitable { .. }));
    assert!(v.to_string().contains("no such budget"), "{v}");
}

#[test]
fn the_load_time_sweep_finds_every_dangling_reference_before_generation() {
    let b = book();
    let good = [
        BudgetRef::by_name("carry range"),
        BudgetRef::distance(4.0),
        BudgetRef::free(),
    ];
    assert!(b.check(good.iter()).is_ok());

    let bad = [
        BudgetRef::by_name("carry range"),
        BudgetRef::by_name("ghost"),
    ];
    assert!(matches!(
        b.check(bad.iter()),
        Err(BudgetError::Dangling { .. })
    ));
}

// ---------------------------------------------------------------------------------------------
// A budget is an Object, so the rest of the machinery already works on it
// ---------------------------------------------------------------------------------------------

#[test]
fn a_budget_is_a_core_class_and_needs_no_special_case() {
    // ⚠ `/Core/Budget` is in the tier-1 tree that `with_core` registers, so a project references one
    // without re-declaring the core — and a `Kind` bounded at it behaves like any other.
    let r = ClassRegistry::with_core();
    assert!(r.contains(&BudgetBound::class_path()));
    let k = Kind::<BudgetBound>::new(&r, class("/Core/Budget")).unwrap();
    assert!(k.is_a(&r, &class("/Core/Object")));
}

#[test]
fn a_budget_describes_itself_the_way_a_developer_named_it() {
    let b = book();
    assert_eq!(
        b.by_name("lava crossing").unwrap().to_string(),
        "lava crossing (3 hearts)"
    );
    assert_eq!(
        b.by_name("air supply").unwrap().to_string(),
        "air supply (90 s)"
    );
    assert_eq!(
        Budget::anonymous(Cost::distance(4.0)).to_string(),
        "4 m (4 m)",
        "an inline cost names itself after its own value"
    );
}

#[test]
fn a_free_move_is_expressible_without_a_book_entry() {
    // A stag that costs nothing should not need a budget declared for zero.
    let b = BudgetBook::new();
    let stag = Components::new().with(Attached::new(Component::FastTravel {
        network: "stagways".into(),
        cost: Some(BudgetRef::free()),
        unlocked_by: Rule::Always,
    }));
    let Some(Component::FastTravel { cost, .. }) = stag.enabled().next() else {
        panic!("a network");
    };
    assert_eq!(cost.as_ref().unwrap().open(&b).unwrap().remaining(), 0.0);
}
