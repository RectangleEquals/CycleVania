//! **Parallel groups** — sibling slots that all exist, whose relative order is unconstrained, all
//! accessible from the group's predecessor and all leading to its successor.
//!
//! ```text
//!                         ┌ Wing A ┐
//!               ┌────────▶│ Flooded│────────┐
//!    ┌ Precursor┐         └────────┘        ▼   ┌ Capstone ┐
//!    │  ⊛ Item  │═══╪                    ═══╪══▶│  ⊛ Boss  │
//!    └──────────┘   └────────▶┌ Wing B ┐───▶┘   └──────────┘
//!                             │ Burning│
//!                             └────────┘
//! ```
//!
//! # Series-parallel, not a general graph, and the restriction is load-bearing
//!
//! ⚠ **Series-parallel keeps a well-defined *before/after* for every pair of slots**, which is exactly
//! what the ordering validation that makes **symbolic grants** safe depends on. A general graph makes
//! *"does the precursor provably precede this?"* undecidable by inspection, and the symbolic-grant
//! feature goes with it.
//!
//! So this is not a simplification anyone should later "fix". The editor draws a general graph and
//! **rejects it with that explanation** — a better failure than a compile error, and the reason
//! [`Ordering::Incomparable`] exists as a value rather than as a panic.
//!
//! # A permutable run is a parallel group of length *n*
//!
//! ⚠ **Do not build both.** *"These three rooms in any order"* and *"these three wings in parallel"* are
//! the same statement about ordering, and two vocabularies for it would have to be kept in agreement
//! forever by people who did not know the other existed.
//!
//! # Forking is also default behaviour, and these are not the same thing
//!
//! | | What it is | Controlled by |
//! |---|---|---|
//! | **default topology** | the algorithm forks, loops and reconverges on its own | dials — `cycle_density`, branchiness |
//! | **stated forks** | *"exactly two wings here, rejoining at the capstone"* | a parallel group |
//!
//! ⚠ **A spine is one way to *state* a fork. It is not what makes forks possible.** Conflating them
//! would make a project that wanted loops believe it needed a spine.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// How two slots are ordered relative to each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ordering {
    /// The first provably precedes the second.
    Before,
    /// The first provably follows the second.
    After,
    /// Both are in the same parallel group: neither precedes the other, and that is **stated**, not
    /// unknown.
    ///
    /// ⚠ **Distinct from [`Incomparable`](Ordering::Incomparable) on purpose.** *"Deliberately
    /// unordered"* is a design decision a symbolic grant can reason about; *"the graph cannot say"* is
    /// a defect. Collapsing them would let an undecidable graph pass as an intentional one.
    Unordered,
    /// ⚠ **The graph is not series-parallel**, so the question has no answer by inspection.
    Incomparable,
}

impl fmt::Display for Ordering {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Ordering::Before => "before",
            Ordering::After => "after",
            Ordering::Unordered => "unordered (a parallel group)",
            Ordering::Incomparable => "incomparable — the graph is not series-parallel",
        })
    }
}

/// Why a spine's ordering is not series-parallel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotSeriesParallel {
    /// A slot appears in more than one group.
    SlotInTwoGroups { slot: String },
    /// A group names a slot that does not exist.
    UnknownSlot { slot: String },
    /// A group's predecessor or successor is inside the group itself.
    SelfBounded { group: String, slot: String },
    /// Two groups overlap in the sequence without nesting.
    ///
    /// ⚠ **The general-graph case, and the one the editor draws.** Overlapping-but-not-nested is what
    /// a developer produces when they wire wings to each other rather than through the reconvergence,
    /// and it is precisely the shape that destroys the before/after relation.
    Interleaved { a: String, b: String },
}

impl fmt::Display for NotSeriesParallel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotSeriesParallel::SlotInTwoGroups { slot } => {
                write!(f, "slot {slot:?} is in two parallel groups")
            }
            NotSeriesParallel::UnknownSlot { slot } => {
                write!(f, "no slot named {slot:?}")
            }
            NotSeriesParallel::SelfBounded { group, slot } => write!(
                f,
                "group {group:?} is bounded by {slot:?}, which is inside it"
            ),
            NotSeriesParallel::Interleaved { a, b } => write!(
                f,
                "groups {a:?} and {b:?} overlap without nesting — the spine is a general graph, and \
                 a general graph has no provable before/after for every pair of slots, which is what \
                 symbolic grants are checked against"
            ),
        }
    }
}

impl std::error::Error for NotSeriesParallel {}

/// Sibling slots that all exist and are mutually unordered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParallelGroup {
    /// A name for reports.
    pub name: String,
    /// The slot every member is reachable from.
    pub predecessor: String,
    /// The slot every member leads to.
    pub successor: String,
    /// The members, in authored order — which is **not** a placement order.
    pub members: Vec<String>,
}

impl ParallelGroup {
    /// A group between two slots.
    pub fn new(
        name: impl Into<String>,
        predecessor: impl Into<String>,
        successor: impl Into<String>,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        ParallelGroup {
            name: name.into(),
            predecessor: predecessor.into(),
            successor: successor.into(),
            members: members.into_iter().map(Into::into).collect(),
        }
    }

    /// Is this slot a member?
    pub fn contains(&self, slot: &str) -> bool {
        self.members.iter().any(|m| m == slot)
    }

    /// ⚠ **A group of one is a plain slot, and a group of none is nothing at all.**
    ///
    /// Worth asking, because a group that degenerated to one member through relaxation should stop
    /// being reported as a fork — a developer reading *"parallel group: 1 wing"* learns nothing except
    /// that something went wrong somewhere else.
    pub fn is_degenerate(&self) -> bool {
        self.members.len() < 2
    }
}

/// A spine's slot sequence plus its parallel groups, with the ordering question answerable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeriesParallel {
    /// The main-line slots in order. Group members are **not** in here.
    line: Vec<String>,
    groups: Vec<ParallelGroup>,
}

impl SeriesParallel {
    /// An empty sequence.
    pub fn new() -> Self {
        SeriesParallel::default()
    }

    /// Append a main-line slot.
    pub fn slot(mut self, name: impl Into<String>) -> Self {
        self.line.push(name.into());
        self
    }

    /// Add a parallel group.
    pub fn group(mut self, g: ParallelGroup) -> Self {
        self.groups.push(g);
        self
    }

    /// The main line, in order.
    pub fn line(&self) -> &[String] {
        &self.line
    }

    /// Every group.
    pub fn groups(&self) -> &[ParallelGroup] {
        &self.groups
    }

    /// Position of a main-line slot.
    fn index_of(&self, slot: &str) -> Option<usize> {
        self.line.iter().position(|s| s == slot)
    }

    /// The group a slot belongs to, if any.
    pub fn group_of(&self, slot: &str) -> Option<&ParallelGroup> {
        self.groups.iter().find(|g| g.contains(slot))
    }

    /// Check the whole structure is series-parallel.
    ///
    /// ⚠ **Every failure names the slots involved.** *"Not series-parallel"* on its own is a message a
    /// developer cannot act on, and the editor renders this text beside the offending wire.
    pub fn validate(&self) -> Result<(), NotSeriesParallel> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for g in &self.groups {
            for m in &g.members {
                if !seen.insert(m.as_str()) {
                    return Err(NotSeriesParallel::SlotInTwoGroups { slot: m.clone() });
                }
            }
            for bound in [&g.predecessor, &g.successor] {
                if g.contains(bound) {
                    return Err(NotSeriesParallel::SelfBounded {
                        group: g.name.clone(),
                        slot: bound.clone(),
                    });
                }
                if self.index_of(bound).is_none() {
                    return Err(NotSeriesParallel::UnknownSlot {
                        slot: bound.clone(),
                    });
                }
            }
        }

        // Spans must nest or be disjoint — never interleave.
        let mut spans: Vec<(usize, usize, &str)> = Vec::new();
        for g in &self.groups {
            let (a, b) = (
                self.index_of(&g.predecessor).expect("checked above"),
                self.index_of(&g.successor).expect("checked above"),
            );
            spans.push((a.min(b), a.max(b), g.name.as_str()));
        }
        for (i, (a0, a1, an)) in spans.iter().enumerate() {
            for (b0, b1, bn) in spans.iter().skip(i + 1) {
                let disjoint = a1 <= b0 || b1 <= a0;
                let nested = (a0 <= b0 && b1 <= a1) || (b0 <= a0 && a1 <= b1);
                if !disjoint && !nested {
                    return Err(NotSeriesParallel::Interleaved {
                        a: (*an).to_string(),
                        b: (*bn).to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Does `a` provably precede `b`?
    ///
    /// ⚠ **This is the question symbolic grants are checked against**, and the reason the structure is
    /// restricted at all. Every pair must get `Before`, `After` or `Unordered`; `Incomparable` means
    /// the spine should have been rejected at author time.
    pub fn order(&self, a: &str, b: &str) -> Ordering {
        if a == b {
            return Ordering::Unordered;
        }
        if let (Some(ga), Some(gb)) = (self.group_of(a), self.group_of(b)) {
            if ga.name == gb.name {
                return Ordering::Unordered;
            }
        }
        let pos = |slot: &str| -> Option<usize> {
            if let Some(i) = self.index_of(slot) {
                return Some(i);
            }
            // A group member sits between its bounds; use the predecessor's position plus a half
            // step, which the integer comparison below models as "after the predecessor, before the
            // successor".
            self.group_of(slot)
                .and_then(|g| self.index_of(&g.predecessor))
        };
        match (pos(a), pos(b)) {
            (Some(ia), Some(ib)) if ia < ib => Ordering::Before,
            (Some(ia), Some(ib)) if ia > ib => Ordering::After,
            (Some(_), Some(_)) => {
                // Same anchor position: one is a group member and the other its predecessor, or both
                // are members of different groups sharing bounds.
                match (self.group_of(a), self.group_of(b)) {
                    (Some(_), None) => Ordering::After,
                    (None, Some(_)) => Ordering::Before,
                    (Some(_), Some(_)) => Ordering::Unordered,
                    (None, None) => Ordering::Incomparable,
                }
            }
            _ => Ordering::Incomparable,
        }
    }

    /// Every slot, main line and group members alike.
    pub fn slots(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for s in &self.line {
            out.push(s.as_str());
            for g in self.groups.iter().filter(|g| &g.predecessor == s) {
                out.extend(g.members.iter().map(String::as_str));
            }
        }
        out
    }

    /// ⚠ **Is every pair of slots orderable?** The property symbolic grants depend on, checked rather
    /// than assumed — `validate` proves the *structure* is series-parallel, and this proves the
    /// consequence the feature actually needs.
    pub fn every_pair_is_orderable(&self) -> Result<(), (String, String)> {
        let slots = self.slots();
        for (i, a) in slots.iter().enumerate() {
            for b in slots.iter().skip(i + 1) {
                if self.order(a, b) == Ordering::Incomparable {
                    return Err(((*a).to_string(), (*b).to_string()));
                }
            }
        }
        Ok(())
    }

    /// How many slots each group contributes, for the capacity arithmetic.
    pub fn group_sizes(&self) -> BTreeMap<&str, usize> {
        self.groups
            .iter()
            .map(|g| (g.name.as_str(), g.members.len()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Precursor ▶ { wing_a, wing_b } ▶ capstone.
    fn wings() -> SeriesParallel {
        SeriesParallel::new()
            .slot("precursor")
            .slot("capstone")
            .group(ParallelGroup::new(
                "wings",
                "precursor",
                "capstone",
                ["wing_a", "wing_b"],
            ))
    }

    #[test]
    fn a_two_wing_group_with_a_shared_capstone_is_series_parallel() {
        let sp = wings();
        assert_eq!(sp.validate(), Ok(()));
        assert_eq!(
            sp.slots(),
            vec!["precursor", "wing_a", "wing_b", "capstone"]
        );
    }

    #[test]
    fn neither_wing_is_ordered_against_the_other_and_that_is_stated() {
        let sp = wings();
        assert_eq!(sp.order("wing_a", "wing_b"), Ordering::Unordered);
        assert_eq!(sp.order("wing_b", "wing_a"), Ordering::Unordered);
    }

    #[test]
    fn both_wings_follow_the_precursor_and_precede_the_capstone() {
        let sp = wings();
        for wing in ["wing_a", "wing_b"] {
            assert_eq!(sp.order(wing, "precursor"), Ordering::After, "{wing}");
            assert_eq!(sp.order(wing, "capstone"), Ordering::Before, "{wing}");
            assert_eq!(sp.order("precursor", wing), Ordering::Before, "{wing}");
        }
    }

    #[test]
    fn every_pair_is_orderable_which_is_what_symbolic_grants_need() {
        assert_eq!(wings().every_pair_is_orderable(), Ok(()));
    }

    #[test]
    fn a_permutable_run_is_a_parallel_group_of_length_n() {
        // ⚠ Do not build both — this *is* "these three rooms in any order".
        let sp = SeriesParallel::new()
            .slot("start")
            .slot("end")
            .group(ParallelGroup::new(
                "any_order",
                "start",
                "end",
                ["a", "b", "c"],
            ));
        assert_eq!(sp.validate(), Ok(()));
        for (a, b) in [("a", "b"), ("b", "c"), ("a", "c")] {
            assert_eq!(sp.order(a, b), Ordering::Unordered);
        }
        assert_eq!(sp.group_sizes()["any_order"], 3);
    }

    #[test]
    fn a_slot_in_two_groups_is_rejected_by_name() {
        let sp = SeriesParallel::new()
            .slot("a")
            .slot("b")
            .slot("c")
            .group(ParallelGroup::new("g1", "a", "b", ["x"]))
            .group(ParallelGroup::new("g2", "b", "c", ["x"]));
        assert_eq!(
            sp.validate(),
            Err(NotSeriesParallel::SlotInTwoGroups { slot: "x".into() })
        );
    }

    #[test]
    fn a_group_bounded_by_its_own_member_is_rejected() {
        let sp = SeriesParallel::new()
            .slot("a")
            .slot("b")
            .group(ParallelGroup::new("g", "a", "b", ["a", "x"]));
        assert!(matches!(
            sp.validate(),
            Err(NotSeriesParallel::SelfBounded { .. })
        ));
    }

    #[test]
    fn a_group_bounded_by_a_slot_that_does_not_exist_is_rejected() {
        let sp =
            SeriesParallel::new()
                .slot("a")
                .group(ParallelGroup::new("g", "a", "nowhere", ["x"]));
        assert_eq!(
            sp.validate(),
            Err(NotSeriesParallel::UnknownSlot {
                slot: "nowhere".into()
            })
        );
    }

    #[test]
    fn interleaved_groups_are_rejected_with_the_explanation_the_editor_shows() {
        // ⚠ The general-graph case: wings wired to each other rather than through the reconvergence.
        let sp = SeriesParallel::new()
            .slot("a")
            .slot("b")
            .slot("c")
            .slot("d")
            .group(ParallelGroup::new("g1", "a", "c", ["x"]))
            .group(ParallelGroup::new("g2", "b", "d", ["y"]));
        let err = sp.validate().unwrap_err();
        assert!(matches!(err, NotSeriesParallel::Interleaved { .. }));
        let text = err.to_string();
        assert!(text.contains("general graph"));
        assert!(
            text.contains("symbolic grants"),
            "the rejection must say why the restriction exists: {text}"
        );
    }

    #[test]
    fn nested_groups_are_fine_and_disjoint_ones_are_too() {
        let nested = SeriesParallel::new()
            .slot("a")
            .slot("b")
            .slot("c")
            .slot("d")
            .group(ParallelGroup::new("outer", "a", "d", ["x"]))
            .group(ParallelGroup::new("inner", "b", "c", ["y"]));
        assert_eq!(nested.validate(), Ok(()));

        let disjoint = SeriesParallel::new()
            .slot("a")
            .slot("b")
            .slot("c")
            .slot("d")
            .group(ParallelGroup::new("g1", "a", "b", ["x"]))
            .group(ParallelGroup::new("g2", "c", "d", ["y"]));
        assert_eq!(disjoint.validate(), Ok(()));
    }

    #[test]
    fn unordered_and_incomparable_are_different_answers() {
        // ⚠ "Deliberately unordered" is a decision; "the graph cannot say" is a defect. A checker that
        // conflated them would let an undecidable spine pass as an intentional one.
        assert_ne!(Ordering::Unordered, Ordering::Incomparable);
        assert!(Ordering::Incomparable
            .to_string()
            .contains("not series-parallel"));
        assert!(Ordering::Unordered.to_string().contains("parallel group"));
    }

    #[test]
    fn a_group_that_relaxed_down_to_one_member_stops_claiming_to_be_a_fork() {
        let g = ParallelGroup::new("wings", "a", "b", ["only"]);
        assert!(g.is_degenerate());
        assert!(!ParallelGroup::new("wings", "a", "b", ["x", "y"]).is_degenerate());
    }

    #[test]
    fn a_slot_nobody_declared_is_incomparable_rather_than_silently_ordered() {
        let sp = wings();
        assert_eq!(sp.order("ghost", "capstone"), Ordering::Incomparable);
        assert_eq!(sp.order("precursor", "ghost"), Ordering::Incomparable);
    }

    #[test]
    fn a_plain_series_spine_orders_every_pair() {
        let sp = SeriesParallel::new().slot("a").slot("b").slot("c");
        assert_eq!(sp.validate(), Ok(()));
        assert_eq!(sp.order("a", "c"), Ordering::Before);
        assert_eq!(sp.order("c", "a"), Ordering::After);
        assert_eq!(sp.every_pair_is_orderable(), Ok(()));
    }
}
