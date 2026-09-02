//! **The schedule-rule consumer** — where a declared [`ScheduleRule`] actually changes the fill, and
//! where a relaxed [`Preference`] gets reported.
//!
//! ⚠ **A lever that changes nothing is not a feature.** [`satisfy`](crate::satisfy) makes that true of
//! every `Constraint`; this module makes it true of the two vocabularies that constrain a **pool and an
//! order** rather than a single candidate — which is why they could not live there. A `Constraint`
//! answers *"may this go here?"*; a `ScheduleRule` answers *"may this go **at all, yet**?"*.
//!
//! | Form | Consumed as |
//! |---|---|
//! | `PlacedAfter` | orders the fill, and holds an item back until its target is placed and the sphere gap is met |
//! | `ExclusiveWith` | blocks co-placement — once one is in, the other leaves the pool |
//! | `Supersedes` | retires the **base** from the pool once its successor is placed |
//! | `SpherePin` | excludes the item outside the pinned sphere range |
//!
//! # `Supersedes` retires, and does not merely deprioritise
//!
//! ⚠ **A Longshot beside a Hookshot is one pickup too many, not variety.** The base leaves the pool
//! entirely once its successor lands, because a world containing both teaches a player that the upgrade
//! was pointless. That makes ordering matter: a base placed *first* is fine — the successor supersedes
//! it later — but a base placed *after* its successor is the bug this rule exists to make impossible.
//!
//! # A relaxed preference is reported, and an omitted `Optional` is not
//!
//! ⚠ **The distinction is the difference between a report and noise.** `PREFERRED` promised something
//! and did not deliver it, so a developer needs to know. `OPTIONAL` promised nothing, so reporting its
//! absence would flood the report with non-events and train them to ignore it — which costs more than
//! not having the report at all.

use crate::object::ObjectId;
use crate::placement::{Preference, ScheduleRule};
use std::collections::BTreeMap;
use std::fmt;

/// Why an item is not placeable right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Held {
    /// Its `PlacedAfter` target has not been placed.
    AwaitingTarget { target: ObjectId },
    /// Its target is placed, but not far enough back in the progression.
    GapNotMet { target: ObjectId, gap: (u32, u32) },
    /// Something it is exclusive with is already in the world.
    ExcludedBy { other: ObjectId },
    /// Its successor is already placed, so it is retired.
    Superseded { by: ObjectId },
    /// The current sphere is outside its pin.
    OutsideSpherePin { min: u32, max: u32, sphere: u32 },
}

impl Held {
    /// ⚠ **Can this item ever become placeable again?**
    ///
    /// The difference between *"not yet"* and *"never"*, and it decides whether the scheduler keeps
    /// paying to re-test the item every round. `Superseded` and `ExcludedBy` are permanent within a
    /// world; the other three dissolve as the fill progresses.
    pub fn is_permanent(&self) -> bool {
        matches!(self, Held::Superseded { .. } | Held::ExcludedBy { .. })
    }
}

impl fmt::Display for Held {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Held::AwaitingTarget { .. } => write!(f, "its PlacedAfter target is not placed yet"),
            Held::GapNotMet { gap, .. } => write!(
                f,
                "its target is placed, but the sphere gap {}..{} is not met",
                gap.0, gap.1
            ),
            Held::ExcludedBy { .. } => write!(f, "something it is ExclusiveWith is already placed"),
            Held::Superseded { .. } => write!(f, "its successor is placed, so the base is retired"),
            Held::OutsideSpherePin { min, max, sphere } => {
                write!(f, "sphere {sphere} is outside its pin of {min}..{max}")
            }
        }
    }
}

/// What has gone into the world so far, and at which sphere.
///
/// ⚠ **The sphere is carried per placement rather than globally**, because `PlacedAfter`'s gap is a
/// distance *in progression* between two specific items. A single "current sphere" would answer a
/// different question, and answer it wrongly the moment two items were placed in the same round.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Placed {
    at: BTreeMap<ObjectId, u32>,
}

impl Placed {
    /// Nothing placed.
    pub fn new() -> Self {
        Placed::default()
    }

    /// Record a placement.
    pub fn record(&mut self, item: ObjectId, sphere: u32) {
        self.at.insert(item, sphere);
    }

    /// Which sphere something went in at.
    pub fn sphere_of(&self, item: ObjectId) -> Option<u32> {
        self.at.get(&item).copied()
    }

    /// Is it in the world?
    pub fn contains(&self, item: ObjectId) -> bool {
        self.at.contains_key(&item)
    }

    /// How many.
    pub fn len(&self) -> usize {
        self.at.len()
    }

    /// Nothing yet.
    pub fn is_empty(&self) -> bool {
        self.at.is_empty()
    }
}

/// Applies schedule rules to a pool.
#[derive(Clone, Debug, Default)]
pub struct Sequencer {
    /// Which item supersedes which base.
    successors: BTreeMap<ObjectId, ObjectId>,
}

impl Sequencer {
    /// A sequencer that knows about no upgrades.
    pub fn new() -> Self {
        Sequencer::default()
    }

    /// Learn the rules attached to one item, so the reverse lookups exist before the fill starts.
    ///
    /// ⚠ **`Supersedes` needs a reverse index and the others do not.** *"Is this base retired?"* is
    /// asked of the **base**, whose own rules say nothing about it — the statement lives on the
    /// successor. Without this pass the base would have to be re-scanned against every other item's
    /// rules on every round.
    pub fn learn(&mut self, item: ObjectId, rules: &[ScheduleRule]) {
        for r in rules {
            if let ScheduleRule::Supersedes { base } = r {
                self.successors.insert(*base, item);
            }
        }
    }

    /// Why this item cannot be placed at this sphere, if it cannot.
    ///
    /// ⚠ **Permanent reasons are checked first.** Reporting *"awaiting its target"* for an item that
    /// has already been retired would send a developer looking for an ordering bug that is not there.
    pub fn holds(
        &self,
        item: ObjectId,
        rules: &[ScheduleRule],
        placed: &Placed,
        sphere: u32,
    ) -> Option<Held> {
        if let Some(by) = self.successors.get(&item) {
            if placed.contains(*by) {
                return Some(Held::Superseded { by: *by });
            }
        }
        for r in rules {
            if let ScheduleRule::ExclusiveWith { other } = r {
                if placed.contains(*other) {
                    return Some(Held::ExcludedBy { other: *other });
                }
            }
        }
        for r in rules {
            match r {
                ScheduleRule::PlacedAfter { target, gap } => {
                    let Some(at) = placed.sphere_of(*target) else {
                        return Some(Held::AwaitingTarget { target: *target });
                    };
                    let delta = sphere.saturating_sub(at);
                    if delta < gap.0 || delta > gap.1 {
                        return Some(Held::GapNotMet {
                            target: *target,
                            gap: *gap,
                        });
                    }
                }
                ScheduleRule::SpherePin { min, max } => {
                    if sphere < *min || sphere > *max {
                        return Some(Held::OutsideSpherePin {
                            min: *min,
                            max: *max,
                            sphere,
                        });
                    }
                }
                ScheduleRule::ExclusiveWith { .. } | ScheduleRule::Supersedes { .. } => {}
            }
        }
        None
    }

    /// The subset of a pool that can go in at this sphere.
    pub fn placeable<'a>(
        &self,
        pool: &'a BTreeMap<ObjectId, Vec<ScheduleRule>>,
        placed: &Placed,
        sphere: u32,
    ) -> Vec<&'a ObjectId> {
        pool.iter()
            .filter(|(id, rules)| self.holds(**id, rules, placed, sphere).is_none())
            .map(|(id, _)| id)
            .collect()
    }

    /// Items that will never be placeable in this world, and why.
    ///
    /// ⚠ **Retirement is an outcome a developer should see**, not a silent shrinking of the pool. A
    /// base that never appeared because its upgrade did is correct; a base that never appeared because
    /// of an ordering mistake looks identical from the world alone.
    pub fn retired(
        &self,
        pool: &BTreeMap<ObjectId, Vec<ScheduleRule>>,
        placed: &Placed,
        sphere: u32,
    ) -> BTreeMap<ObjectId, Held> {
        pool.iter()
            .filter_map(|(id, rules)| {
                self.holds(*id, rules, placed, sphere)
                    .filter(Held::is_permanent)
                    .map(|h| (*id, h))
            })
            .collect()
    }
}

/// One row of the relaxations report.
#[derive(Clone, Debug, PartialEq)]
pub struct Relaxed {
    /// What was asked for.
    pub preference: Preference,
    /// Where it was asked.
    pub item: ObjectId,
    /// What the solver did instead.
    pub instead: String,
}

impl fmt::Display for Relaxed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "relaxed {:?} — {}",
            self.preference.constraint, self.instead
        )
    }
}

/// Honours preferences by weight, and records the ones it could not.
///
/// ⚠ **`Required` never reaches this type.** A requirement that could be relaxed by a reporting
/// mechanism would not be a requirement, so [`honour`](Preferences::honour) refuses it rather than
/// recording it — the failure belongs to [`escalate`](crate::escalate), where a failed requirement is
/// a `Failure` with a layer that can fix it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preferences {
    relaxed: Vec<Relaxed>,
}

impl Preferences {
    /// Nothing relaxed yet.
    pub fn new() -> Self {
        Preferences::default()
    }

    /// Record that a preference could not be met.
    ///
    /// Answers whether the relaxation was legal: a `Required` preference may not be relaxed, and
    /// saying so at the call site is what keeps the report from quietly containing one.
    pub fn honour(&mut self, item: ObjectId, p: &Preference, instead: impl Into<String>) -> bool {
        if !p.is_relaxable() {
            return false;
        }
        if p.relaxation_is_reportable() {
            self.relaxed.push(Relaxed {
                preference: p.clone(),
                item,
                instead: instead.into(),
            });
        }
        true
    }

    /// Every relaxation a developer should see.
    pub fn rows(&self) -> &[Relaxed] {
        &self.relaxed
    }

    /// How many.
    pub fn len(&self) -> usize {
        self.relaxed.len()
    }

    /// Nothing to report.
    pub fn is_empty(&self) -> bool {
        self.relaxed.is_empty()
    }

    /// Order preferences the solver should try hardest to keep, first.
    ///
    /// ⚠ **Stable, and by weight only.** Two preferences of equal weight keep their authored order,
    /// because a tie broken by anything else would make the world depend on a hash order the developer
    /// never chose and the seed does not explain.
    pub fn by_weight(prefs: &[Preference]) -> Vec<&Preference> {
        let mut out: Vec<&Preference> = prefs.iter().collect();
        out.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeKind;
    use crate::placement::Constraint;

    fn oid(s: &str) -> ObjectId {
        ObjectId::derived("item", s)
    }

    fn pool(entries: &[(&str, Vec<ScheduleRule>)]) -> BTreeMap<ObjectId, Vec<ScheduleRule>> {
        entries.iter().map(|(n, r)| (oid(n), r.clone())).collect()
    }

    #[test]
    fn placed_after_holds_an_item_until_its_target_is_in() {
        let rules = vec![ScheduleRule::PlacedAfter {
            target: oid("key"),
            gap: (1, 3),
        }];
        let s = Sequencer::new();
        let mut placed = Placed::new();
        assert_eq!(
            s.holds(oid("door"), &rules, &placed, 2),
            Some(Held::AwaitingTarget { target: oid("key") })
        );
        placed.record(oid("key"), 1);
        assert_eq!(s.holds(oid("door"), &rules, &placed, 2), None);
    }

    #[test]
    fn placed_after_enforces_the_gap_at_both_ends() {
        let rules = vec![ScheduleRule::PlacedAfter {
            target: oid("key"),
            gap: (2, 3),
        }];
        let s = Sequencer::new();
        let mut placed = Placed::new();
        placed.record(oid("key"), 1);
        // Too soon.
        assert!(matches!(
            s.holds(oid("door"), &rules, &placed, 2),
            Some(Held::GapNotMet { .. })
        ));
        // In range.
        assert_eq!(s.holds(oid("door"), &rules, &placed, 3), None);
        // Too late — a gap is a window, not a floor.
        assert!(matches!(
            s.holds(oid("door"), &rules, &placed, 9),
            Some(Held::GapNotMet { .. })
        ));
    }

    #[test]
    fn exclusive_with_removes_the_other_once_one_is_in() {
        let rules = vec![ScheduleRule::ExclusiveWith { other: oid("beam") }];
        let s = Sequencer::new();
        let mut placed = Placed::new();
        assert_eq!(s.holds(oid("wave"), &rules, &placed, 0), None);
        placed.record(oid("beam"), 0);
        assert_eq!(
            s.holds(oid("wave"), &rules, &placed, 0),
            Some(Held::ExcludedBy { other: oid("beam") })
        );
    }

    #[test]
    fn supersedes_retires_the_base_and_is_asked_of_the_base() {
        // ⚠ The statement lives on the successor; the question is asked of the base.
        let mut s = Sequencer::new();
        s.learn(
            oid("longshot"),
            &[ScheduleRule::Supersedes {
                base: oid("hookshot"),
            }],
        );
        let mut placed = Placed::new();
        assert_eq!(
            s.holds(oid("hookshot"), &[], &placed, 0),
            None,
            "before the upgrade lands, the base is perfectly placeable"
        );
        placed.record(oid("longshot"), 2);
        assert_eq!(
            s.holds(oid("hookshot"), &[], &placed, 3),
            Some(Held::Superseded {
                by: oid("longshot")
            }),
            "a Longshot beside a Hookshot is one pickup too many, not variety"
        );
    }

    #[test]
    fn sphere_pin_excludes_outside_its_range() {
        let rules = vec![ScheduleRule::SpherePin { min: 3, max: 5 }];
        let s = Sequencer::new();
        let placed = Placed::new();
        assert!(matches!(
            s.holds(oid("capstone"), &rules, &placed, 1),
            Some(Held::OutsideSpherePin { .. })
        ));
        assert_eq!(s.holds(oid("capstone"), &rules, &placed, 4), None);
        assert!(matches!(
            s.holds(oid("capstone"), &rules, &placed, 6),
            Some(Held::OutsideSpherePin { .. })
        ));
    }

    #[test]
    fn a_permanent_hold_is_reported_ahead_of_a_temporary_one() {
        // ⚠ Otherwise a retired base reads as an ordering bug that is not there.
        let mut s = Sequencer::new();
        s.learn(oid("up"), &[ScheduleRule::Supersedes { base: oid("base") }]);
        let mut placed = Placed::new();
        placed.record(oid("up"), 1);
        let rules = vec![ScheduleRule::PlacedAfter {
            target: oid("never"),
            gap: (1, 1),
        }];
        assert_eq!(
            s.holds(oid("base"), &rules, &placed, 2),
            Some(Held::Superseded { by: oid("up") })
        );
    }

    #[test]
    fn permanence_separates_not_yet_from_never() {
        assert!(Held::Superseded { by: oid("x") }.is_permanent());
        assert!(Held::ExcludedBy { other: oid("x") }.is_permanent());
        assert!(!Held::AwaitingTarget { target: oid("x") }.is_permanent());
        assert!(!Held::OutsideSpherePin {
            min: 1,
            max: 2,
            sphere: 0
        }
        .is_permanent());
    }

    #[test]
    fn the_pool_shrinks_and_the_retirements_are_visible() {
        let mut s = Sequencer::new();
        s.learn(oid("up"), &[ScheduleRule::Supersedes { base: oid("base") }]);
        let p = pool(&[
            ("base", vec![]),
            ("up", vec![]),
            ("other", vec![ScheduleRule::SpherePin { min: 9, max: 9 }]),
        ]);
        let mut placed = Placed::new();
        assert_eq!(
            s.placeable(&p, &placed, 0).len(),
            2,
            "base and up; not other"
        );
        placed.record(oid("up"), 0);
        assert_eq!(s.placeable(&p, &placed, 1).len(), 1);
        let retired = s.retired(&p, &placed, 1);
        assert_eq!(retired.len(), 1);
        assert!(retired.contains_key(&oid("base")));
        assert!(
            !retired.contains_key(&oid("other")),
            "a sphere pin is not yet, not never"
        );
    }

    #[test]
    fn a_preferred_relaxation_is_reported_and_an_optional_one_is_not() {
        let c = Constraint::WithinScope {
            scope: NodeKind::Space,
        };
        let mut r = Preferences::new();
        assert!(r.honour(
            oid("a"),
            &Preference::preferred(c.clone(), 0.8),
            "nearest available"
        ));
        assert_eq!(r.len(), 1);
        assert!(r.honour(oid("b"), &Preference::optional(c.clone(), 0.8), "skipped"));
        assert_eq!(
            r.len(),
            1,
            "an OPTIONAL that did not appear was never promised; reporting it is noise"
        );
    }

    #[test]
    fn a_required_preference_cannot_be_relaxed_at_all() {
        let c = Constraint::AloneInScope {
            scope: NodeKind::Space,
        };
        let mut r = Preferences::new();
        assert!(
            !r.honour(oid("a"), &Preference::required(c), "anything"),
            "a requirement a reporting mechanism could relax would not be a requirement"
        );
        assert!(r.is_empty());
    }

    #[test]
    fn preferences_are_ordered_by_weight_and_ties_keep_their_authored_order() {
        let c = Constraint::WithinScope {
            scope: NodeKind::Space,
        };
        let prefs = vec![
            Preference::preferred(c.clone(), 0.2),
            Preference::preferred(c.clone(), 0.9),
            Preference::preferred(c.clone(), 0.9),
        ];
        let ordered = Preferences::by_weight(&prefs);
        assert_eq!(ordered[0].weight, 0.9);
        assert_eq!(ordered[2].weight, 0.2);
        // ⚠ Stable: the two 0.9s stay in authored order, or the world depends on something the seed
        // does not explain.
        assert!(std::ptr::eq(ordered[0], &prefs[1]));
        assert!(std::ptr::eq(ordered[1], &prefs[2]));
    }

    #[test]
    fn a_relaxation_row_says_what_happened_instead() {
        let c = Constraint::WithinScope {
            scope: NodeKind::Space,
        };
        let mut r = Preferences::new();
        r.honour(
            oid("a"),
            &Preference::preferred(c, 0.5),
            "placed in the adjoining Area",
        );
        assert!(r.rows()[0]
            .to_string()
            .contains("placed in the adjoining Area"));
    }

    #[test]
    fn every_hold_explains_itself() {
        let holds = [
            Held::AwaitingTarget { target: oid("x") },
            Held::GapNotMet {
                target: oid("x"),
                gap: (1, 2),
            },
            Held::ExcludedBy { other: oid("x") },
            Held::Superseded { by: oid("x") },
            Held::OutsideSpherePin {
                min: 1,
                max: 2,
                sphere: 5,
            },
        ];
        for h in holds {
            assert!(!h.to_string().is_empty());
        }
    }

    #[test]
    fn placements_are_tracked_per_item_rather_than_as_one_current_sphere() {
        let mut p = Placed::new();
        assert!(p.is_empty());
        p.record(oid("a"), 0);
        p.record(oid("b"), 4);
        assert_eq!(p.sphere_of(oid("a")), Some(0));
        assert_eq!(p.sphere_of(oid("b")), Some(4));
        assert_eq!(p.sphere_of(oid("c")), None);
        assert_eq!(p.len(), 2);
        assert!(p.contains(oid("a")));
    }
}
