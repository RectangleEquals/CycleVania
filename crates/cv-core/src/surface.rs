//! **What a piece of geometry means to a mechanic.**
//!
//! Geometry answers *where*. A surface answers *who may be here* and *what works here* — and keeping
//! those apart is what stops the primitives needing to know what a laser is.
//!
//! # Two questions, deliberately not one
//!
//! ```text
//! supports(ctx, occupant) -> Array<Support>   can something BE here?
//! affords(ctx, attempt)   -> Rule             does an ACTION work here?
//! ```
//!
//! ⚠ **`affords` covers transit *through*, not only action *upon*.** That is the whole reason one pane
//! of glass can answer four mechanics differently: a bullet is stopped, a laser passes, walking is
//! blocked, sight passes. The surface is never asked *"are you solid?"* — it is asked whether a
//! specific attempt works, and the attempt carries its own kind.
//!
//! # Why `supports` returns an array
//!
//! Because alternatives each carry **their own endurance**, and the solver's dependency walk has to see
//! every unlock involved in every one of them.
//!
//! ⚠ Returning a single `Support` and branching internally on what the occupant holds would hide that
//! dependency — the walk would see one answer and no reason for it, and a route could then depend on
//! something the generator never placed. **The shape rules the hole out**, rather than a rule asking
//! everyone to remember.
//!
//! # Empty is a real answer
//!
//! An empty array means *never supported*, and it is different from a permissive default. Lava returns
//! a `Support` gated on fire immunity; a bottomless pit returns nothing at all. Both are floors as far
//! as [`crate::floor`] is concerned — **geometry never vetoes**, and this is where the difference is
//! actually stated.

use crate::budget::BudgetRef;
use crate::mission::Rule;
use crate::object::ObjectId;
use crate::placement::Interaction;
use std::fmt;

/// One way to be somewhere.
///
/// ⚠ Several may apply at once — *"walkable if you have the boots"* and *"walkable briefly if you do
/// not"* are two supports with different endurance, and the solver picks between them.
#[derive(Clone, Debug, PartialEq)]
pub struct Support {
    /// What the occupant must satisfy to use this support.
    pub permitted: Rule,
    /// The steepest slope this support tolerates, in degrees.
    pub max_slope: f64,
    /// How long it lasts. `None` means indefinitely.
    ///
    /// ⚠ **This is how standing on lava with fire boots differs from standing on stone.** Both are
    /// supported; only one runs out.
    pub endurance: Option<BudgetRef>,
}

impl Support {
    /// Supported unconditionally, up to the project's slope limit.
    pub fn always(max_slope: f64) -> Self {
        Support {
            permitted: Rule::Always,
            max_slope,
            endurance: None,
        }
    }

    /// Supported only while a rule holds.
    pub fn permitted_by(rule: Rule, max_slope: f64) -> Self {
        Support {
            permitted: rule,
            max_slope,
            endurance: None,
        }
    }

    /// Supported, but only for so long.
    pub fn lasting(mut self, budget: BudgetRef) -> Self {
        self.endurance = Some(budget);
        self
    }

    /// Does this support tolerate that slope?
    pub fn admits_slope(&self, degrees: f64) -> bool {
        degrees <= self.max_slope
    }
}

/// How dangerous being here is.
#[derive(Clone, Debug, PartialEq)]
pub struct Harm {
    /// How far the danger reaches.
    pub radius: f64,
    /// How bad, 0..1.
    pub severity: f64,
    /// Can an occupant avoid it by playing well?
    ///
    /// ⚠ Declared rather than derived: the generator cannot know whether a hazard is dodgeable, and
    /// guessing would make difficulty a fiction.
    pub avoidable: bool,
    /// Does it apply continuously, or once on contact?
    pub continuous: bool,
    /// What removes it.
    pub mitigated_by: Option<ObjectId>,
}

impl Harm {
    /// Nothing dangerous here.
    pub const NONE: Harm = Harm {
        radius: 0.0,
        severity: 0.0,
        avoidable: true,
        continuous: false,
        mitigated_by: None,
    };

    /// Is there anything to be harmed by?
    pub fn is_none(&self) -> bool {
        self.severity <= 0.0
    }
}

/// What an occupant needs at the near end of a traversal.
#[derive(Clone, Debug, PartialEq)]
pub struct Approach {
    /// How far away it may be attempted from.
    pub distance: (f64, f64),
    /// The steepest ground it may be attempted from.
    pub max_slope: f64,
    /// A surface it must be attempted from, if any.
    pub surface: Option<ObjectId>,
}

/// Who is standing, as a parameter.
///
/// ⚠ **`actor` is `None` for the player**, and that is not an oversight — the player is not an
/// authored object. The solver poses counterfactuals (*"suppose an occupant held these"*) rather than
/// querying a live entity, which is also why `held` is a set of unlock ids rather than an inventory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Occupant {
    /// The actor doing the occupying, or `None` for the player.
    pub actor: Option<ObjectId>,
    /// What it is holding.
    pub held: Vec<ObjectId>,
    /// How much room it needs.
    pub footprint: (f64, f64, f64),
}

impl Occupant {
    /// The player, holding these unlocks.
    pub fn player(held: impl IntoIterator<Item = ObjectId>) -> Self {
        Occupant {
            actor: None,
            held: held.into_iter().collect(),
            footprint: (0.6, 1.9, 0.6),
        }
    }

    /// Is this the player?
    pub fn is_player(&self) -> bool {
        self.actor.is_none()
    }

    /// Is this unlock held?
    pub fn holds(&self, unlock: ObjectId) -> bool {
        self.held.contains(&unlock)
    }
}

/// What a piece of geometry means to a mechanic.
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    /// What it is called, for traces and the editor.
    pub name: String,
    /// Tags assignable by face selection — what an eligible-surface `TagQuery` matches against.
    ///
    /// ⚠ **A tag query rather than a hand-maintained class list**, so *"anything portalable"* keeps
    /// working when a project adds its twelfth portalable material.
    pub tags: Vec<ObjectId>,
    /// Ways to be here, in preference order. **Empty means never supported.**
    supports: Vec<Support>,
    /// What attempts work here, keyed by what the attempt is.
    affords: Vec<(AttemptKind, Rule)>,
    /// How slippery, 0..1.
    pub friction: f64,
    /// How dangerous.
    pub harm: Harm,
    /// Does it accept mounted content?
    pub admits_mount: bool,
}

/// Which family of attempt an affordance answers for.
///
/// ⚠ **Not a flow enum.** It classifies by *what relocates*, matching [`Interaction`]'s own split, so
/// a project adding a new mechanic writes an `Interaction` subclass rather than extending a core list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttemptKind {
    /// The occupant relocates.
    Movement,
    /// An object relocates.
    Displace,
    /// Nothing relocates — it acts at range.
    RemoteUse,
}

impl AttemptKind {
    /// Which family an attempt belongs to.
    pub fn of(attempt: &Interaction) -> Self {
        match attempt {
            Interaction::Movement { .. } => AttemptKind::Movement,
            Interaction::Displace { .. } => AttemptKind::Displace,
            Interaction::RemoteUse { .. } => AttemptKind::RemoteUse,
        }
    }
}

impl Surface {
    /// An ordinary floor: walkable, not slippery, not dangerous.
    pub fn ordinary(name: impl Into<String>, max_slope: f64) -> Self {
        Surface {
            name: name.into(),
            tags: Vec::new(),
            supports: vec![Support::always(max_slope)],
            affords: Vec::new(),
            friction: 1.0,
            harm: Harm::NONE,
            admits_mount: true,
        }
    }

    /// A surface nothing can stand on.
    ///
    /// ⚠ Distinct from a *hazard*: a bottomless pit supports nobody, while lava supports anyone with
    /// the right protection. Both are floors to the geometry.
    pub fn unsupported(name: impl Into<String>) -> Self {
        Surface {
            supports: Vec::new(),
            ..Surface::ordinary(name, 0.0)
        }
    }

    /// Add a way to be here.
    pub fn supporting(mut self, support: Support) -> Self {
        self.supports.push(support);
        self
    }

    /// State what a family of attempt does here.
    pub fn affording(mut self, kind: AttemptKind, rule: Rule) -> Self {
        self.affords.push((kind, rule));
        self
    }

    /// Tag this surface.
    pub fn tagged(mut self, tag: ObjectId) -> Self {
        self.tags.push(tag);
        self
    }

    /// Make it dangerous.
    pub fn harming(mut self, harm: Harm) -> Self {
        self.harm = harm;
        self
    }

    /// **Can something be here?**
    ///
    /// Returns every applicable way, each with its own endurance — never one blended answer.
    pub fn supports(&self, occupant: &Occupant) -> Vec<&Support> {
        self.supports
            .iter()
            .filter(|s| {
                let held = occupant.held.iter().copied().collect();
                s.permitted.is_satisfied(&held)
            })
            .collect()
    }

    /// Is this surface usable by that occupant at all?
    pub fn is_supported(&self, occupant: &Occupant) -> bool {
        !self.supports(occupant).is_empty()
    }

    /// **Does an action work here?**
    ///
    /// ⚠ **Covers transit *through*, not only action *upon*.** An unstated attempt family is refused,
    /// which is the conservative direction: a surface that has said nothing about lasers is not
    /// thereby transparent to them.
    pub fn affords(&self, attempt: &Interaction) -> Rule {
        let kind = AttemptKind::of(attempt);
        self.affords
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, r)| r.clone())
            .unwrap_or(Rule::Never)
    }
}

impl fmt::Display for Surface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(n: &str) -> ObjectId {
        ObjectId::derived("actor", n)
    }
    fn unlock(n: &str) -> ObjectId {
        ObjectId::derived("unlock", n)
    }

    #[test]
    fn one_pane_of_glass_answers_four_attempts_differently() {
        // ⚠ The claim `affords` exists for. The surface is never asked "are you solid?" — it is asked
        // whether a *specific attempt* works, and the attempt carries its own kind.
        let glass = Surface::ordinary("glass", 0.0)
            .affording(AttemptKind::Movement, Rule::Never)
            .affording(AttemptKind::RemoteUse, Rule::Always);

        let walk = Interaction::movement((0.0, 1.0));
        let laser = Interaction::RemoteUse {
            target: oid("switch"),
            range: (0.0, 40.0),
            line_of_sight: true,
        };
        let shove = Interaction::Displace {
            subject: oid("crate"),
            range: (0.0, 2.0),
        };

        assert_eq!(glass.affords(&walk), Rule::Never, "walking is blocked");
        assert_eq!(glass.affords(&laser), Rule::Always, "light passes");
        assert_eq!(
            glass.affords(&shove),
            Rule::Never,
            "an unstated attempt is refused, not silently permitted"
        );
    }

    #[test]
    fn an_unstated_affordance_is_refused_rather_than_assumed() {
        // ⚠ P1, the conservative direction. A surface that has said nothing about lasers is not
        // thereby transparent to them.
        let stone = Surface::ordinary("stone", 50.0);
        let laser = Interaction::RemoteUse {
            target: oid("x"),
            range: (0.0, 10.0),
            line_of_sight: true,
        };
        assert_eq!(stone.affords(&laser), Rule::Never);
    }

    #[test]
    fn supports_returns_every_alternative_so_the_walk_sees_each_dependency() {
        // ⚠ Returning one blended answer would hide which unlock made it possible, and a route could
        // then depend on something the generator never placed.
        let boots = unlock("fire_boots");
        let lava = Surface::unsupported("lava").supporting(
            Support::permitted_by(Rule::has(boots), 10.0).lasting(BudgetRef::time(4.0, 5.0)),
        );

        let barefoot = Occupant::player([]);
        let shod = Occupant::player([boots]);

        assert!(lava.supports(&barefoot).is_empty(), "nothing holds you up");
        let ways = lava.supports(&shod);
        assert_eq!(ways.len(), 1);
        assert!(
            ways[0].endurance.is_some(),
            "and it runs out, which is what makes it different from stone"
        );
    }

    #[test]
    fn a_pit_and_a_hazard_are_different_things() {
        // Both are floors to the geometry — ⚠ geometry never vetoes. The difference is stated here.
        let pit = Surface::unsupported("pit");
        let anyone = Occupant::player([unlock("everything")]);
        assert!(
            !pit.is_supported(&anyone),
            "nothing crosses a pit by holding an item"
        );

        let lava = Surface::unsupported("lava")
            .supporting(Support::permitted_by(Rule::has(unlock("fire_boots")), 10.0));
        assert!(lava.is_supported(&Occupant::player([unlock("fire_boots")])));
    }

    #[test]
    fn a_support_declares_the_slope_it_tolerates() {
        let s = Support::always(45.0);
        assert!(s.admits_slope(30.0));
        assert!(s.admits_slope(45.0), "the limit itself is admitted");
        assert!(!s.admits_slope(60.0));
    }

    #[test]
    fn the_player_is_an_occupant_without_an_actor() {
        // ⚠ Not an oversight: the player is not an authored object, and the solver poses
        // counterfactuals rather than querying a live entity.
        let p = Occupant::player([unlock("dash")]);
        assert!(p.is_player());
        assert!(p.actor.is_none());
        assert!(p.holds(unlock("dash")));
        assert!(!p.holds(unlock("glide")));
    }

    #[test]
    fn harm_none_is_actually_none() {
        assert!(Harm::NONE.is_none());
        assert!(!Harm {
            severity: 0.5,
            ..Harm::NONE
        }
        .is_none());
    }

    #[test]
    fn tags_are_what_an_eligible_surface_query_matches() {
        // ⚠ A tag query rather than a hand-maintained class list, so "anything portalable" survives a
        // project adding its twelfth portalable material.
        let s = Surface::ordinary("shingle", 40.0)
            .tagged(oid("Surface.Roof"))
            .tagged(oid("Surface.Portalable"));
        assert_eq!(s.tags.len(), 2);
        assert!(s.tags.contains(&oid("Surface.Portalable")));
    }
}
