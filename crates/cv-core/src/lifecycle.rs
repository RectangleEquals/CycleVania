//! **The lifecycle events, and the item economy they act on.**
//!
//! # Events are not hooks, and the difference is the return value
//!
//! > **A hook is asked a question. An event is told what happened.**
//!
//! Every hook returns something the solver consumes — a rule, a verdict, a footprint. Every event
//! returns **nothing**, and that is exactly what makes it safe to fire during a search: an event
//! cannot influence the decision that produced it, so firing one mid-search cannot make the search
//! depend on evaluation order.
//!
//! ⚠ **This is why they are the Event Graph's nodes rather than the Schematic's.** A node with no
//! return value has nowhere to connect an output, so the visual surface for the two is genuinely
//! different — and a developer who tries to read a result from `on_placed` finds there is no pin to
//! read it from, rather than finding a subtly wrong answer.
//!
//! # `on_rejected` carries the verdict
//!
//! ⚠ **Rejection is the channel, not the exception.** The event carries the [`Verdict`] that refused
//! the candidate, because *"it did not fit"* is useless and *"over budget by 6.2"* is actionable. A
//! mechanic that wants to know why its own placement failed reads it here.

use crate::judge::Verdict;
use crate::object::ObjectId;
use std::fmt;

/// A point in a placement's life at which content is *told* something.
///
/// ⚠ **Never returns a value**, by construction. See the module docs for why that is the whole
/// distinction between an event and a hook.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The solver is considering this position.
    Proposed,
    /// It went here.
    Placed,
    /// A candidate position was refused, **with the verdict that refused it**.
    Rejected { why: Verdict },
    /// An occupant got the thing and kept what it grants.
    Obtained,
    /// A component of this object changed.
    ComponentChanged { component: ObjectId },
    /// Generation is over and nothing further will move.
    Finalized,
    /// Return to the state before generation.
    ///
    /// ⚠ Fired on regeneration, so content holding derived state has one defined place to drop it.
    /// Without this, a re-run inherits the previous world's leftovers and stops being reproducible.
    Reset,
}

impl Event {
    /// The name a trace and the Event Graph use.
    pub fn name(&self) -> &'static str {
        match self {
            Event::Proposed => "on_proposed",
            Event::Placed => "on_placed",
            Event::Rejected { .. } => "on_rejected",
            Event::Obtained => "on_obtained",
            Event::ComponentChanged { .. } => "on_component_changed",
            Event::Finalized => "on_finalized",
            Event::Reset => "reset",
        }
    }

    /// Can this fire more than once for one object in one pass?
    ///
    /// ⚠ `Proposed` and `Rejected` fire once per *candidate*, which is why a mechanic must not
    /// accumulate state in them — the search tries many positions and keeps one.
    pub fn is_per_candidate(&self) -> bool {
        matches!(self, Event::Proposed | Event::Rejected { .. })
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Rejected { why } => write!(f, "on_rejected({why})"),
            other => f.write_str(other.name()),
        }
    }
}

/// Whether and how a supply comes back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Replenish {
    /// Once taken, gone.
    #[default]
    Never,
    /// Restored when the occupant re-enters the scope.
    OnReenter,
    /// Restored from a placed source — a dispenser, a spring.
    FromSource,
    /// Restored on a clock.
    OnTimer,
}

/// How much of something exists, and whether it comes back.
///
/// ⚠ **Supply is not the same question as the lattice.** A consumable that runs out does not make a
/// world unsolvable if it replenishes, and one that never replenishes is a budget the solver must
/// respect. Keeping the two apart is what lets a soft gate be a magnitude rather than a lock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    /// How many exist at the start.
    pub initial: u32,
    /// The most that can be held at once.
    pub max: u32,
    /// Is it used up on use?
    pub consumable: bool,
    /// Whether and how it comes back.
    pub replenishes: Replenish,
}

impl Quantity {
    /// A single permanent thing — the common case for a progression item.
    pub fn single() -> Self {
        Quantity {
            initial: 1,
            max: 1,
            consumable: false,
            replenishes: Replenish::Never,
        }
    }

    /// A consumable supply.
    pub fn consumable(initial: u32, max: u32, replenishes: Replenish) -> Self {
        Quantity {
            initial,
            max,
            consumable: true,
            replenishes,
        }
    }

    /// Can this run out permanently?
    ///
    /// ⚠ **The question the softlock pass cares about.** A consumable that never replenishes is the
    /// only kind that can strand a player by being spent, so it is the only kind the un-softlockable
    /// proof has to reason about as a finite resource.
    pub fn can_be_exhausted(&self) -> bool {
        self.consumable && self.replenishes == Replenish::Never
    }
}

impl Default for Quantity {
    fn default() -> Self {
        Quantity::single()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(n: &str) -> ObjectId {
        ObjectId::derived("actor", n)
    }

    #[test]
    fn a_rejection_event_carries_the_verdict_that_caused_it() {
        // ⚠ Rejection is the channel, not the exception. "It did not fit" is useless; "over budget by
        // 6.2" is something a mechanic can act on.
        let e = Event::Rejected {
            why: Verdict::over_budget(6.2),
        };
        assert_eq!(e.name(), "on_rejected");
        assert!(format!("{e}").contains("6.2"));
    }

    #[test]
    fn per_candidate_events_are_marked_because_they_fire_many_times() {
        // ⚠ The search tries many positions and keeps one. A mechanic accumulating state in
        // `on_proposed` would be counting attempts, not placements.
        assert!(Event::Proposed.is_per_candidate());
        assert!(Event::Rejected {
            why: Verdict::blocked(oid("pillar"))
        }
        .is_per_candidate());
        assert!(!Event::Placed.is_per_candidate());
        assert!(!Event::Finalized.is_per_candidate());
    }

    #[test]
    fn every_event_names_itself_for_the_event_graph() {
        let all = [
            Event::Proposed,
            Event::Placed,
            Event::Rejected {
                why: Verdict::accepted(0.0),
            },
            Event::Obtained,
            Event::ComponentChanged {
                component: oid("hinge"),
            },
            Event::Finalized,
            Event::Reset,
        ];
        let names: Vec<&str> = all.iter().map(Event::name).collect();
        assert_eq!(
            names,
            vec![
                "on_proposed",
                "on_placed",
                "on_rejected",
                "on_obtained",
                "on_component_changed",
                "on_finalized",
                "reset",
            ]
        );
    }

    #[test]
    fn only_an_unreplenishing_consumable_can_strand_a_player() {
        // ⚠ What the softlock pass has to treat as finite. Everything else either comes back or was
        // never spent in the first place.
        assert!(Quantity::consumable(3, 3, Replenish::Never).can_be_exhausted());
        assert!(!Quantity::consumable(3, 3, Replenish::OnReenter).can_be_exhausted());
        assert!(!Quantity::single().can_be_exhausted());
    }

    #[test]
    fn a_progression_item_is_a_single_permanent_thing_by_default() {
        let q = Quantity::default();
        assert_eq!(q.initial, 1);
        assert!(!q.consumable);
        assert_eq!(q.replenishes, Replenish::Never);
    }
}
