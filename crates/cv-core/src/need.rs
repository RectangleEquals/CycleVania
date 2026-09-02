//! **`PlacementNeed`** — what `requires()` returns, and the vocabulary it returns it in.
//!
//! ⚠ **`requires()` has no expressible answer without this.** A hook that must say *"put me where there
//! is a two-metre gap and a flat wall behind it"* needs words for *gap* and *flat wall*; until those
//! exist the hook can only return nothing, and a mechanic that asks for nothing gets placed anywhere.
//!
//! # The core satisfies named requests; it does not understand them
//!
//! ⚠ **Generic constraint satisfaction over a named vocabulary** is the whole design. The core does not
//! know what a grapple *is* — it knows how to find a spot with a gap of at least 8 metres and a flat
//! patch at the far end. That is what lets a project invent a mechanic the core has never heard of and
//! still have it placed correctly.
//!
//! The vocabulary is therefore **open-ended by intent**: [`Spatial::Named`] carries a project's own
//! request, and a solver that cannot satisfy one **says so** rather than ignoring it.
//!
//! # Why the three forms are not one
//!
//! | Form | Asks for | Fails as |
//! |---|---|---|
//! | [`PlacementNeed::Actor`] | *something else must exist, accessible* | an unplaceable dependency |
//! | [`PlacementNeed::Clearance`] | *this volume must stay empty* | an intersection |
//! | [`PlacementNeed::BlocksTraversal`] | *put me on an edge of this kind and close it* | a deleted region |
//!
//! ⚠ The third is **the inverse of the other two** — it does not ask for space, it asks for an *edge*.
//! Without it a barrier can only be authored as geometry, which violates P2 by construction: it deletes
//! a region rather than gating it.

use crate::collision::CollisionBody;
use crate::geometry::Face;
use crate::judge::Route;
use crate::path::ClassPath;
use std::fmt;

/// One spatial request, in the vocabulary the core and content share.
///
/// ⚠ **Open-ended on purpose.** The listed forms are what the core can satisfy directly; `Named` is how
/// a project asks for something the core has never heard of. A closed set would make the core's
/// imagination the ceiling on what content can express.
#[derive(Clone, Debug, PartialEq)]
pub enum Spatial {
    /// A horizontal gap of at least this size.
    MinGap { distance: f64 },
    /// A vertical step or ledge within a range.
    ///
    /// ⚠ **A range, not a minimum.** *"A ledge I can climb"* has a ceiling as well as a floor — a
    /// two-metre step is not a better version of a one-metre step, it is an impassable one.
    Height { min: f64, max: f64 },
    /// A flat wall, floor or ceiling patch.
    FlatSurface {
        width: f64,
        height: f64,
        facing: Face,
    },
    /// A point with visibility and accessibility properties, each in `0..1`.
    AnchorPoint { visibility: f64, accessibility: f64 },
    /// No obstruction along a path or volume.
    PathClearance { envelope: CollisionBody },
    /// Several distinct viewing angles onto a target.
    ///
    /// ⚠ **Puzzle readability**, and the one primitive that is about the *player's understanding*
    /// rather than the player's body. A puzzle whose parts cannot be seen together cannot be solved by
    /// reasoning, only by exhaustion.
    MultiVantage { count: u32 },
    /// An arbitrary reserved sub-volume — the escape hatch.
    VolumeReservation { shape: CollisionBody },
    /// A project's own request, by name.
    ///
    /// ⚠ **A solver that cannot satisfy one must say so.** Silently ignoring an unrecognised request
    /// would place the content anyway and report success, which is worse than refusing: the developer
    /// sees a world that generated and no reason to look at it.
    Named { name: String, magnitude: f64 },
}

impl Spatial {
    /// A horizontal gap.
    pub fn min_gap(distance: f64) -> Self {
        Spatial::MinGap { distance }
    }

    /// A vertical range. Reversed inputs are sorted rather than rejected.
    pub fn height(min: f64, max: f64) -> Self {
        Spatial::Height {
            min: min.min(max),
            max: min.max(max),
        }
    }

    /// A flat patch facing a direction.
    pub fn flat_surface(width: f64, height: f64, facing: Face) -> Self {
        Spatial::FlatSurface {
            width,
            height,
            facing,
        }
    }

    /// A point with visibility and accessibility, each clamped to `0..1`.
    pub fn anchor_point(visibility: f64, accessibility: f64) -> Self {
        Spatial::AnchorPoint {
            visibility: visibility.clamp(0.0, 1.0),
            accessibility: accessibility.clamp(0.0, 1.0),
        }
    }

    /// Several distinct viewing angles.
    pub fn multi_vantage(count: u32) -> Self {
        Spatial::MultiVantage { count }
    }

    /// A project's own request.
    pub fn named(name: impl Into<String>, magnitude: f64) -> Self {
        Spatial::Named {
            name: name.into(),
            magnitude,
        }
    }

    /// The vocabulary word this request uses.
    ///
    /// ⚠ **The key a solver dispatches on**, and the string a *"cannot satisfy"* diagnostic names. A
    /// request the solver does not recognise is reported under this word, so the developer sees the
    /// name they wrote.
    pub fn word(&self) -> &str {
        match self {
            Spatial::MinGap { .. } => "min_gap",
            Spatial::Height { .. } => "height",
            Spatial::FlatSurface { .. } => "flat_surface",
            Spatial::AnchorPoint { .. } => "anchor_point",
            Spatial::PathClearance { .. } => "path_clearance",
            Spatial::MultiVantage { .. } => "multi_vantage",
            Spatial::VolumeReservation { .. } => "volume_reservation",
            Spatial::Named { name, .. } => name,
        }
    }

    /// Is this a request the core knows how to satisfy itself?
    ///
    /// ⚠ `false` for [`Spatial::Named`] — which is not a failure, it is the open half of the
    /// vocabulary. It is the *solver's* job to say whether it can honour one, and to fail loudly when
    /// it cannot.
    pub fn is_core_vocabulary(&self) -> bool {
        !matches!(self, Spatial::Named { .. })
    }
}

impl fmt::Display for Spatial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Spatial::MinGap { distance } => write!(f, "a gap of at least {distance}"),
            Spatial::Height { min, max } => write!(f, "a step between {min} and {max}"),
            Spatial::FlatSurface {
                width,
                height,
                facing,
            } => write!(f, "a flat {width}×{height} patch facing {facing:?}"),
            Spatial::AnchorPoint {
                visibility,
                accessibility,
            } => write!(f, "an anchor seen {visibility}, reached {accessibility}"),
            Spatial::PathClearance { .. } => f.write_str("an unobstructed path"),
            Spatial::MultiVantage { count } => write!(f, "{count} distinct vantages"),
            Spatial::VolumeReservation { .. } => f.write_str("a reserved volume"),
            Spatial::Named { name, magnitude } => write!(f, "{name} of {magnitude}"),
        }
    }
}

/// What a placement needs in order to work.
#[derive(Clone, Debug, PartialEq)]
pub enum PlacementNeed {
    /// Something carrying this component must exist, accessible by this route.
    ///
    /// ⚠ **`having` is a class, not an instance** — *"a torch"*, not *"that torch"*. Naming an instance
    /// would make the need unsatisfiable before that instance is placed, which is most of the run.
    Actor {
        having: ClassPath,
        route: Option<Route>,
    },
    /// This volume must stay empty, and these spatial requests must hold.
    ///
    /// ⚠ **The volume and the requests are both here** because *"somewhere with a gap this wide"* and
    /// *"and nothing in the way once I am there"* are one question. Splitting them would let a solver
    /// satisfy each separately at different spots.
    Clearance {
        volume: CollisionBody,
        spatial: Vec<Spatial>,
    },
    /// *"Place me **on** an edge of this kind, and close it."*
    BlocksTraversal { matching: ClassPath },
}

impl PlacementNeed {
    /// Needs something carrying a component, anywhere.
    pub fn actor(having: ClassPath) -> Self {
        PlacementNeed::Actor {
            having,
            route: None,
        }
    }

    /// Needs something carrying a component, accessible by a route.
    pub fn actor_via(having: ClassPath, route: Route) -> Self {
        PlacementNeed::Actor {
            having,
            route: Some(route),
        }
    }

    /// Needs empty space.
    pub fn clearance(volume: CollisionBody) -> Self {
        PlacementNeed::Clearance {
            volume,
            spatial: Vec::new(),
        }
    }

    /// Needs a spot answering these spatial requests.
    pub fn spatial(requests: impl IntoIterator<Item = Spatial>) -> Self {
        PlacementNeed::Clearance {
            volume: CollisionBody::empty(),
            spatial: requests.into_iter().collect(),
        }
    }

    /// Add a spatial request.
    pub fn asking(mut self, request: Spatial) -> Self {
        if let PlacementNeed::Clearance { spatial, .. } = &mut self {
            spatial.push(request);
        }
        self
    }

    /// Needs an edge of this kind to sit on.
    pub fn blocks(matching: ClassPath) -> Self {
        PlacementNeed::BlocksTraversal { matching }
    }

    /// The form's name.
    pub fn form(&self) -> &'static str {
        match self {
            PlacementNeed::Actor { .. } => "NeedsActor",
            PlacementNeed::Clearance { .. } => "NeedsClearance",
            PlacementNeed::BlocksTraversal { .. } => "BlocksTraversal",
        }
    }

    /// Every spatial request this need carries.
    pub fn requests(&self) -> &[Spatial] {
        match self {
            PlacementNeed::Clearance { spatial, .. } => spatial,
            _ => &[],
        }
    }

    /// Every request the core has no built-in answer for.
    ///
    /// ⚠ **What a solver reports rather than skips.** An unrecognised request that produced no
    /// diagnostic would place the content anyway and call it a success.
    pub fn unrecognised(&self) -> Vec<&str> {
        self.requests()
            .iter()
            .filter(|r| !r.is_core_vocabulary())
            .map(Spatial::word)
            .collect()
    }

    /// Does satisfying this need require another piece of content to exist first?
    ///
    /// ⚠ **The dependency-walk question.** A need that plants a source has to be seen by the fill, or
    /// the generator gates something on content it never placed.
    pub fn depends_on(&self) -> Option<&ClassPath> {
        match self {
            PlacementNeed::Actor { having, .. } => Some(having),
            PlacementNeed::BlocksTraversal { matching } => Some(matching),
            PlacementNeed::Clearance { .. } => None,
        }
    }
}

impl fmt::Display for PlacementNeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlacementNeed::Actor { having, route } => {
                let how = if route.is_some() {
                    " by a required route"
                } else {
                    ""
                };
                write!(f, "something carrying {having}{how}")
            }
            PlacementNeed::Clearance { spatial, .. } if spatial.is_empty() => {
                f.write_str("empty space")
            }
            PlacementNeed::Clearance { spatial, .. } => {
                let parts: Vec<String> = spatial.iter().map(Spatial::to_string).collect();
                write!(f, "a spot with {}", parts.join(", and "))
            }
            PlacementNeed::BlocksTraversal { matching } => write!(f, "an edge of kind {matching}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::BudgetRef;
    use crate::object::ObjectId;
    use crate::shape::Shape;
    use cv_determinism::Vec3;

    fn class(p: &str) -> ClassPath {
        ClassPath::new(p).unwrap()
    }

    // --- the vocabulary -----------------------------------------------------------------------

    #[test]
    fn a_grapple_states_what_it_needs_without_the_core_knowing_what_a_grapple_is() {
        // ⚠ **Generic constraint satisfaction over named requests.** The core finds a spot with a gap
        // and a flat patch; it never learns what the mechanic is. That is what lets a project invent
        // one the core has never heard of.
        let need = PlacementNeed::spatial([
            Spatial::min_gap(8.0),
            Spatial::flat_surface(2.0, 2.0, Face::NegZ),
        ]);
        assert_eq!(need.requests().len(), 2);
        assert!(need.to_string().contains("gap of at least 8"));
        assert!(need.unrecognised().is_empty());
    }

    #[test]
    fn a_project_may_ask_for_something_the_core_has_never_heard_of() {
        // ⚠ The open half of the vocabulary. A closed set would make the core's imagination the
        // ceiling on what content can express.
        let need = PlacementNeed::spatial([Spatial::named("tide_line", 3.0)]);
        assert_eq!(need.requests()[0].word(), "tide_line");
        assert!(!need.requests()[0].is_core_vocabulary());
    }

    #[test]
    fn an_unrecognised_request_is_reportable_rather_than_skippable() {
        // ⚠ Silently ignoring one would place the content anyway and report success — worse than
        // refusing, because the developer sees a world that generated and no reason to look at it.
        let need = PlacementNeed::spatial([
            Spatial::min_gap(4.0),
            Spatial::named("tide_line", 3.0),
            Spatial::named("wind_shadow", 1.0),
        ]);
        assert_eq!(need.unrecognised(), vec!["tide_line", "wind_shadow"]);
    }

    #[test]
    fn a_height_request_is_a_range_because_a_bigger_step_is_not_a_better_one() {
        // ⚠ A two-metre step is not an improved one-metre step; it is an impassable one. A minimum
        // alone would let the solver "improve" a ledge until nobody can climb it.
        let Spatial::Height { min, max } = Spatial::height(0.4, 1.2) else {
            panic!("a height range")
        };
        assert_eq!((min, max), (0.4, 1.2));
        assert_eq!(Spatial::height(1.2, 0.4), Spatial::height(0.4, 1.2));
    }

    #[test]
    fn anchor_properties_are_clamped_because_they_are_fractions() {
        let Spatial::AnchorPoint {
            visibility,
            accessibility,
        } = Spatial::anchor_point(9.0, -2.0)
        else {
            panic!("an anchor")
        };
        assert_eq!((visibility, accessibility), (1.0, 0.0));
    }

    #[test]
    fn multi_vantage_is_about_readability_rather_than_accessibility() {
        // ⚠ The one primitive about the player's *understanding*: a puzzle whose parts cannot be seen
        // together can only be solved by exhaustion.
        let need = PlacementNeed::spatial([Spatial::multi_vantage(3)]);
        assert_eq!(need.requests()[0].word(), "multi_vantage");
        assert!(need.to_string().contains("3 distinct vantages"));
    }

    // --- the three forms ----------------------------------------------------------------------

    #[test]
    fn the_three_forms_ask_three_different_questions() {
        let forms = [
            PlacementNeed::actor(class("/Content/Components/LightSource")),
            PlacementNeed::clearance(CollisionBody::of(Shape::Sphere { radius: 1.0 })),
            PlacementNeed::blocks(class("/Content/Components/VaultDoor")),
        ];
        assert_eq!(
            forms.iter().map(PlacementNeed::form).collect::<Vec<_>>(),
            vec!["NeedsActor", "NeedsClearance", "BlocksTraversal"]
        );
    }

    #[test]
    fn needing_an_actor_names_a_class_and_never_an_instance() {
        // ⚠ *"A torch"*, not *"that torch"*. An instance reference would make the need unsatisfiable
        // until that instance is placed, which is most of the run.
        let need = PlacementNeed::actor(class("/Content/Props/Torch"));
        assert_eq!(need.depends_on(), Some(&class("/Content/Props/Torch")));
    }

    #[test]
    fn a_route_makes_the_dependency_accessible_rather_than_merely_present() {
        // *"A Bomb Flower somewhere in the world"* and *"…within carry range"* are different needs.
        let route = Route::required(
            ObjectId::derived("actor", "here"),
            ObjectId::derived("actor", "flower"),
            BudgetRef::by_name("carry range"),
        );
        let need = PlacementNeed::actor_via(class("/Content/Props/BombFlower"), route);
        assert!(need.to_string().contains("by a required route"));
    }

    #[test]
    fn a_barrier_asks_for_an_edge_and_not_for_space() {
        // ⚠ The inverse of the other two, and P2's guarantee: without it a barrier is authored as
        // geometry, which *deletes* a region rather than gating it.
        let need = PlacementNeed::blocks(class("/Content/Components/TetherAnchor"));
        assert!(need.requests().is_empty(), "it wants no space at all");
        assert!(need.depends_on().is_some(), "but it does name an edge kind");
    }

    #[test]
    fn clearance_carries_its_volume_and_its_requests_together() {
        // ⚠ *"Somewhere with a gap this wide"* and *"nothing in the way once I am there"* are one
        // question; splitting them lets a solver satisfy each at a different spot.
        let need = PlacementNeed::clearance(CollisionBody::of(Shape::Cube {
            extents: Vec3::new(2.0, 3.0, 2.0),
            bevel: 0.0,
        }))
        .asking(Spatial::min_gap(6.0));
        let PlacementNeed::Clearance { volume, spatial } = &need else {
            panic!("clearance")
        };
        assert_eq!(volume.len(), 1);
        assert_eq!(spatial.len(), 1);
    }

    #[test]
    fn only_the_forms_that_name_content_enter_the_dependency_walk() {
        // ⚠ A clearance need depends on nothing being placed; treating it as a dependency would make
        // the fill wait on a thing that does not exist.
        assert_eq!(
            PlacementNeed::clearance(CollisionBody::empty()).depends_on(),
            None
        );
        assert!(PlacementNeed::actor(class("/Content/A"))
            .depends_on()
            .is_some());
        assert!(PlacementNeed::blocks(class("/Content/B"))
            .depends_on()
            .is_some());
    }

    #[test]
    fn a_need_explains_itself_for_a_trace() {
        let need = PlacementNeed::spatial([Spatial::min_gap(8.0), Spatial::multi_vantage(2)]);
        let text = need.to_string();
        assert!(text.starts_with("a spot with"), "{text}");
        assert!(text.contains(", and "), "{text}");
        assert_eq!(
            PlacementNeed::clearance(CollisionBody::empty()).to_string(),
            "empty space"
        );
    }
}
