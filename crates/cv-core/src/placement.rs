//! **Interactions, constraints, preferences, and the classification/role split.**
//!
//! # Interactions are classified by what relocates
//!
//! | Kind | What moves |
//! |---|---|
//! | [`Interaction::Movement`] | **the player** — so it is a directed graph edge |
//! | [`Interaction::Displace`] | **an object**, not the player. Push, carry and throw |
//! | [`Interaction::RemoteUse`] | **nothing** — it acts at range. Sightlines, beams, ballistics |
//!
//! ⚠ **This is what lets one pane of glass answer four mechanics differently.** A surface is not asked
//! *"are you solid?"*; it is asked *"does this attempt work here?"*, and the attempt carries its own
//! kind. Glass stops a bullet, passes a laser, blocks walking and passes sight — four answers, one
//! surface, no flow enum anywhere in the geometry.
//!
//! ⚠ **The hook is `gate()`, never `requires()`.** On an `Actor`, `requires()` means *"what must exist
//! near me"* — a placement demand. Reusing the name for *"what must the occupant hold"* would put two
//! unrelated questions under one word, and both are in the generated palette.
//!
//! # Constraints are hard; preferences are not
//!
//! A [`Constraint`] the solver cannot satisfy means the placement does not happen. A [`Preference`] it
//! cannot satisfy is **relaxed and reported** — never silently dropped, because a relaxation nobody
//! sees is indistinguishable from a bug.
//!
//! # Classification is an input; role is an output
//!
//! > **P3.** `classification` is declared by a developer. `role` is assigned by the solver, *after* the
//! > search, from what the placement actually ran into.
//!
//! A developer cannot know in advance that their statue ends up being the landmark players navigate by
//! — that is a fact about the generated world, not about the statue. Asking them to declare it would be
//! asking them to predict the generator.
//!
//! ⚠ **`PROGRESSION` is the conservative default (P1).** Content whose classification nobody stated may
//! appear in logic, so forgetting to classify something makes the solver *more* careful rather than
//! less. And content accessible **solely** through a relaxation is auto-tagged `BONUS` — enforced
//! structurally, not by a convention someone has to remember.

use crate::budget::BudgetRef;
// ⚠ One `Strictness` for the whole system: the design defines it as *"how hard a spine slot **or
// preference** must hold"*, so a second copy here would be two vocabularies for one idea.
use crate::node::NodeKind;
use crate::object::ObjectId;
use crate::spine::Strictness;
use cv_determinism::Vec3;
use std::fmt;

// ---------------------------------------------------------------------------------------------
// Interaction
// ---------------------------------------------------------------------------------------------

/// A cone of permitted directions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionCone {
    /// The cone's centre direction.
    pub axis: Vec3,
    /// Half-angle in degrees. `180.0` accepts everything.
    pub angle: f64,
}

impl DirectionCone {
    /// Any direction at all.
    pub fn any() -> Self {
        DirectionCone {
            axis: Vec3::new(0.0, 1.0, 0.0),
            angle: 180.0,
        }
    }

    /// A cone about an axis.
    ///
    /// ⚠ Replaces a three-value up/level/down enum, so *"up, level or diagonally up, never down"* is
    /// expressible — which the enum could not say.
    pub fn about(axis: Vec3, angle: f64) -> Self {
        DirectionCone {
            axis: axis.normalized(),
            angle,
        }
    }

    /// Does this direction fall inside the cone?
    pub fn admits(&self, direction: Vec3) -> bool {
        if self.angle >= 180.0 {
            return true;
        }
        let d = direction.normalized();
        let cos = self.axis.dot(d).clamp(-1.0, 1.0);
        cos >= cv_determinism::math::cos(cv_determinism::math::to_radians(self.angle))
    }
}

/// Something an occupant attempts.
///
/// ⚠ **Subclassed by what relocates**, which is the classification that makes one surface answer
/// several mechanics differently.
#[derive(Clone, Debug, PartialEq)]
pub enum Interaction {
    /// **The player** relocates — a directed graph edge.
    Movement {
        /// How far it reaches.
        range: (f64, f64),
        /// Permitted vertical change.
        rise: (f64, f64),
        /// Which directions it admits.
        direction: DirectionCone,
        /// Must the whole attempt keep line of sight?
        line_of_sight: bool,
        /// Must it start from standing?
        from_standing: bool,
    },
    /// **An object** relocates, not the player.
    Displace {
        /// What is being moved.
        subject: ObjectId,
        range: (f64, f64),
    },
    /// **Nothing** relocates — it acts at range.
    RemoteUse {
        /// What it acts upon.
        target: ObjectId,
        range: (f64, f64),
        line_of_sight: bool,
    },
}

impl Interaction {
    /// A plain movement of the given reach.
    pub fn movement(range: (f64, f64)) -> Self {
        Interaction::Movement {
            range,
            rise: (0.0, 0.0),
            direction: DirectionCone::any(),
            line_of_sight: false,
            from_standing: true,
        }
    }

    /// How far this attempt reaches.
    pub fn range(&self) -> (f64, f64) {
        match self {
            Interaction::Movement { range, .. }
            | Interaction::Displace { range, .. }
            | Interaction::RemoteUse { range, .. } => *range,
        }
    }

    /// Does this attempt require an unbroken sightline?
    pub fn needs_line_of_sight(&self) -> bool {
        match self {
            Interaction::Movement { line_of_sight, .. }
            | Interaction::RemoteUse { line_of_sight, .. } => *line_of_sight,
            Interaction::Displace { .. } => false,
        }
    }

    /// Does the **player** end up somewhere else?
    ///
    /// ⚠ The question the accessibility graph asks. A `Displace` moves a crate and changes the world;
    /// it does not move the occupant, so it creates no traversal edge.
    pub fn relocates_occupant(&self) -> bool {
        matches!(self, Interaction::Movement { .. })
    }

    /// What this attempt consumes, if anything.
    ///
    /// Default is nothing: an attempt that spends a resource says so, and the ones that do not are the
    /// common case.
    pub fn consumes(&self) -> Option<&str> {
        None
    }

    /// The attempt in words, for the trace.
    pub fn explain(&self) -> String {
        match self {
            Interaction::Movement { range, .. } => {
                format!("the occupant moves {}–{}", range.0, range.1)
            }
            Interaction::Displace { subject, range } => {
                format!("{subject} is displaced {}–{}", range.0, range.1)
            }
            Interaction::RemoteUse { target, range, .. } => {
                format!("{target} is used at {}–{}", range.0, range.1)
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Constraints, preferences, schedule rules
// ---------------------------------------------------------------------------------------------

/// A hard placement requirement. Unsatisfiable means the placement does not happen.
#[derive(Clone, Debug, PartialEq)]
pub enum Constraint {
    /// No sibling of this kind in the same scope.
    AloneInScope { scope: NodeKind },
    /// At least this far from a named kind.
    ///
    /// ⚠ **A door writes key-to-lock distance here**, because the door names its own unlock and the
    /// key does not know its lock. This is what replaced the refused `progression_locality` dial: a
    /// per-door statement rather than a global knob.
    MinDistanceFrom { kind: ObjectId, budget: BudgetRef },
    /// At most this far from a named kind.
    MaxDistanceFrom { kind: ObjectId, budget: BudgetRef },
    /// Must be mounted on a socket matching these tags.
    MountedOn { accepts: Vec<ObjectId> },
    /// Only inside this kind of scope.
    WithinScope { scope: NodeKind },
    /// Never inside this kind of scope.
    NotWithinScope { scope: NodeKind },
    /// Pin to a range of spheres.
    ///
    /// ⚠ **The first constraint about *pacing* rather than topology**, and the one developers reach
    /// for soonest — *"the capstone must not be accessible before sphere 3"*.
    SpherePin { min: u32, max: u32 },
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constraint::AloneInScope { scope } => write!(f, "alone in its {scope}"),
            Constraint::MinDistanceFrom { kind, .. } => write!(f, "far from {kind}"),
            Constraint::MaxDistanceFrom { kind, .. } => write!(f, "near {kind}"),
            Constraint::MountedOn { .. } => write!(f, "mounted on a matching socket"),
            Constraint::WithinScope { scope } => write!(f, "within a {scope}"),
            Constraint::NotWithinScope { scope } => write!(f, "not within a {scope}"),
            Constraint::SpherePin { min, max } => write!(f, "in spheres {min}–{max}"),
        }
    }
}

/// A soft placement bias.
///
/// ⚠ **Relaxable, and reported when relaxed.** A relaxation nobody sees is indistinguishable from a
/// bug, so the report is not optional decoration — it is what makes *"nothing is loose by accident"*
/// true rather than aspirational.
#[derive(Clone, Debug, PartialEq)]
pub struct Preference {
    /// What is being asked for.
    pub constraint: Constraint,
    /// How hard.
    pub strictness: Strictness,
    /// Relative importance among preferences, 0..1.
    pub weight: f64,
}

impl Preference {
    /// A preference the solver may relax.
    pub fn preferred(constraint: Constraint, weight: f64) -> Self {
        Preference {
            constraint,
            strictness: Strictness::Preferred,
            weight: weight.clamp(0.0, 1.0),
        }
    }

    /// A preference the generator may ignore entirely without reporting it.
    ///
    /// ⚠ Distinct from `Preferred`: absence here is **expected**, so it is not a relaxation and there
    /// is nothing to report. Conflating the two would flood the relaxation report with non-events.
    pub fn optional(constraint: Constraint, weight: f64) -> Self {
        Preference {
            constraint,
            strictness: Strictness::Optional,
            weight: weight.clamp(0.0, 1.0),
        }
    }

    /// A preference that must hold.
    pub fn required(constraint: Constraint) -> Self {
        Preference {
            constraint,
            strictness: Strictness::Required,
            weight: 1.0,
        }
    }

    /// May the solver drop this to make a world?
    pub fn is_relaxable(&self) -> bool {
        self.strictness != Strictness::Required
    }

    /// Must dropping this be reported?
    ///
    /// ⚠ **`Preferred` yes, `Optional` no.** A relaxation nobody sees is indistinguishable from a
    /// bug — but an `Optional` that did not appear was never promised, so reporting it would be noise
    /// that trains a developer to ignore the report.
    pub fn relaxation_is_reportable(&self) -> bool {
        self.strictness == Strictness::Preferred
    }
}

/// Ordering and replacement between pieces of content.
#[derive(Clone, Debug, PartialEq)]
pub enum ScheduleRule {
    /// Place me after this target, with a gap in spheres.
    PlacedAfter { target: ObjectId, gap: (u32, u32) },
    /// Never place me in the same world as this.
    ExclusiveWith { other: ObjectId },
    /// This replaces a base once available — the upgrade relationship.
    ///
    /// ⚠ The base is **retired from the pool** once its successor is placed, rather than both
    /// existing: a Longshot beside a Hookshot is one pickup too many, not variety.
    Supersedes { base: ObjectId },
    /// Pin to a sphere range.
    SpherePin { min: u32, max: u32 },
}

// ---------------------------------------------------------------------------------------------
// Classification and role — the P3 split
// ---------------------------------------------------------------------------------------------

/// What kind of reward something is. **An input the developer declares.**
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemClass {
    /// May appear in logic.
    ///
    /// ⚠ **The conservative default (P1).** Content nobody classified is treated as gating, so a
    /// forgotten classification makes the solver more careful rather than less.
    #[default]
    Progression,
    /// Tunes route difficulty and spends slack. **Never gates.**
    Useful,
    /// Rewards optional exploration. **Never gates.**
    ///
    /// ⚠ Auto-assigned to anything accessible *solely* through a relaxation — enforced structurally,
    /// not by a convention someone remembers to apply.
    Bonus,
    /// Satisfies density. **Never gates.**
    Filler,
}

impl ItemClass {
    /// May content of this class appear in access logic?
    ///
    /// ⚠ Only `PROGRESSION` may. The other three exist precisely so a shop can never gate a
    /// progression item on currency the fill placed for density.
    pub fn may_gate(self) -> bool {
        self == ItemClass::Progression
    }
}

impl fmt::Display for ItemClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ItemClass::Progression => "PROGRESSION",
            ItemClass::Useful => "USEFUL",
            ItemClass::Bonus => "BONUS",
            ItemClass::Filler => "FILLER",
        })
    }
}

/// What a placement turned out to be. **An output, assigned after the search.**
///
/// ⚠ **Never declared.** A developer cannot know their statue becomes the landmark players navigate
/// by — that is a fact about the generated world. Asking them to declare it would be asking them to
/// predict the generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// It answered every hook with nothing. It is scenery.
    Decoration,
    /// Something had to get past it.
    Obstacle,
    /// It became a way through.
    Traversal,
    /// It gated progression.
    Gate,
    /// It became a navigational reference.
    Landmark,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Role::Decoration => "DECORATION",
            Role::Obstacle => "OBSTACLE",
            Role::Traversal => "TRAVERSAL",
            Role::Gate => "GATE",
            Role::Landmark => "LANDMARK",
        })
    }
}

/// What the solver observed about a placement, from which its [`Role`] follows.
///
/// ⚠ **Assigned from what the search ran into**, which is why this is a record of observations rather
/// than a field someone sets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoleEvidence {
    /// A rule somewhere referenced it.
    pub gated_something: bool,
    /// A route passed through it.
    pub carried_a_route: bool,
    /// A route had to go around it.
    pub obstructed_a_route: bool,
    /// It was used as a navigational reference.
    pub navigated_by: bool,
}

impl RoleEvidence {
    /// The role these observations imply.
    ///
    /// ⚠ **Ordered by consequence, most load-bearing first.** A thing that both gates and obstructs is
    /// reported as a `GATE`, because that is the fact a developer needs when asking why a world is
    /// shaped as it is.
    pub fn role(&self) -> Role {
        if self.gated_something {
            Role::Gate
        } else if self.carried_a_route {
            Role::Traversal
        } else if self.navigated_by {
            Role::Landmark
        } else if self.obstructed_a_route {
            Role::Obstacle
        } else {
            // ⚠ *"A thing with no mechanical consequence is decoration."* Not an insult — a report,
            // and the one the unused-content table exists to surface.
            Role::Decoration
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(n: &str) -> ObjectId {
        ObjectId::derived("actor", n)
    }

    // --- interactions ----------------------------------------------------------------------------

    #[test]
    fn one_surface_answers_four_attempts_differently() {
        // ⚠ The reason `affords` takes an `Interaction` rather than a flow enum: the *attempt* carries
        // its kind, so a pane of glass can answer each of these on its own terms with the geometry
        // knowing nothing about lasers.
        let walk = Interaction::movement((0.0, 1.0));
        let shove = Interaction::Displace {
            subject: oid("crate"),
            range: (0.0, 2.0),
        };
        let shoot = Interaction::RemoteUse {
            target: oid("switch"),
            range: (0.0, 40.0),
            line_of_sight: true,
        };

        assert!(walk.relocates_occupant());
        assert!(
            !shove.relocates_occupant(),
            "a crate moves, the player does not"
        );
        assert!(!shoot.relocates_occupant(), "nothing relocates at all");
        assert!(shoot.needs_line_of_sight());
        assert!(!shove.needs_line_of_sight());
    }

    #[test]
    fn a_direction_cone_can_say_what_a_three_value_enum_could_not() {
        // "up, level or diagonally up, never down"
        let up_ish = DirectionCone::about(Vec3::new(0.0, 1.0, 0.0), 90.0);
        assert!(up_ish.admits(Vec3::new(0.0, 1.0, 0.0)), "straight up");
        assert!(
            up_ish.admits(Vec3::new(1.0, 0.05, 0.0)),
            "very nearly level"
        );
        assert!(!up_ish.admits(Vec3::new(0.0, -1.0, 0.0)), "never down");
    }

    #[test]
    fn an_unrestricted_cone_admits_everything() {
        let any = DirectionCone::any();
        for d in [Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, 0.0, 0.0)] {
            assert!(any.admits(d));
        }
    }

    // --- constraints and preferences --------------------------------------------------------------

    #[test]
    fn a_preference_is_relaxable_and_a_requirement_is_not() {
        let c = Constraint::AloneInScope {
            scope: NodeKind::Space,
        };
        assert!(Preference::preferred(c.clone(), 0.5).is_relaxable());
        assert!(!Preference::required(c).is_relaxable());
    }

    #[test]
    fn only_a_broken_promise_is_worth_reporting() {
        // ⚠ `Preferred` was promised and then dropped — report it. `Optional` promised nothing, so
        // reporting its absence would train a developer to ignore the relaxation report.
        let c = Constraint::AloneInScope {
            scope: NodeKind::Space,
        };
        assert!(Preference::preferred(c.clone(), 0.5).relaxation_is_reportable());
        assert!(!Preference::optional(c.clone(), 0.5).relaxation_is_reportable());
        assert!(!Preference::required(c).relaxation_is_reportable());
    }

    #[test]
    fn the_door_states_key_to_lock_distance_because_the_key_cannot() {
        // ⚠ What replaced the refused `progression_locality` dial: a per-door statement, not a knob.
        let c = Constraint::MinDistanceFrom {
            kind: oid("tether"),
            budget: BudgetRef::distance(3.0),
        };
        assert_eq!(format!("{c}"), format!("far from {}", oid("tether")));
    }

    #[test]
    fn preference_weight_cannot_escape_its_range() {
        assert_eq!(
            Preference::preferred(
                Constraint::WithinScope {
                    scope: NodeKind::Area
                },
                7.0
            )
            .weight,
            1.0
        );
    }

    // --- the P3 split -----------------------------------------------------------------------------

    #[test]
    fn forgetting_to_classify_makes_the_solver_more_careful_not_less() {
        // ⚠ P1, the conservative direction. The default must be the one whose failure mode is a
        // needlessly-safe world rather than a softlocked one.
        assert_eq!(ItemClass::default(), ItemClass::Progression);
        assert!(ItemClass::default().may_gate());
    }

    #[test]
    fn only_progression_may_appear_in_logic() {
        // Without this a shop could gate a progression item on currency the fill placed for density.
        assert!(ItemClass::Progression.may_gate());
        for c in [ItemClass::Useful, ItemClass::Bonus, ItemClass::Filler] {
            assert!(!c.may_gate(), "{c} must never gate");
        }
    }

    #[test]
    fn role_is_derived_from_what_the_search_ran_into() {
        // ⚠ An output, never a declaration. A developer cannot predict which statue becomes the
        // landmark, because that is a fact about the generated world.
        assert_eq!(RoleEvidence::default().role(), Role::Decoration);
        assert_eq!(
            RoleEvidence {
                carried_a_route: true,
                ..Default::default()
            }
            .role(),
            Role::Traversal
        );
        assert_eq!(
            RoleEvidence {
                navigated_by: true,
                ..Default::default()
            }
            .role(),
            Role::Landmark
        );
    }

    #[test]
    fn a_thing_that_both_gates_and_obstructs_is_reported_as_a_gate() {
        // Ordered by consequence: the gate is the fact a developer needs when asking why the world
        // is shaped as it is.
        let e = RoleEvidence {
            gated_something: true,
            obstructed_a_route: true,
            carried_a_route: true,
            navigated_by: true,
        };
        assert_eq!(e.role(), Role::Gate);
    }

    #[test]
    fn a_thing_with_no_mechanical_consequence_is_decoration() {
        // Not an insult — a report, and the one the unused-content table exists to surface.
        assert_eq!(RoleEvidence::default().role(), Role::Decoration);
    }
}
