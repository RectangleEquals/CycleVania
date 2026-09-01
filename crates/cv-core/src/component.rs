//! **The eight core components** — where mechanics are actually written.
//!
//! An `Actor` is *the thing*; a component is *the behaviour it owns*. Nothing downstream can be
//! authored without these, because an Actor with no components answers every hook with nothing and is,
//! by the design's own definition, decoration.
//!
//! # Aggregation is a default, not an absence
//!
//! ⚠ The base `Actor` gathers each hook from its **enabled** components, in **attach order**.
//! Overriding replaces that; calling the parent extends it.
//!
//! That default exists because the alternative fails silently. If an Actor had to forward each hook by
//! hand, one missing forwarding line would make a mechanic *do nothing* — no error, no warning, just a
//! component that never gets asked. The editor lints the override case for the same reason.
//!
//! # Two of these are load-bearing in ways that are easy to miss
//!
//! **`CheckpointComponent` is P15's second satisfaction route.** *Any state you can enter, you must be
//! able to leave* has two solutions: refuse every irreversible edge, or take one and guarantee the
//! reset. Without checkpoints the solver must refuse, and a whole class of attractive one-way
//! transitions — a drop into a vault, a collapsing bridge — becomes unbuildable.
//!
//! **`FastTravelComponent` is not cosmetic.** A network **collapses traversal cost across the entire
//! World**, and difficulty here *is* slack spent against a budget. A network the solver cannot see
//! makes every difficulty judgement in the project silently wrong — not slightly off, wrong, because
//! the budget it was measured against no longer describes the world.

use crate::collision::{CollisionBody, CollisionData};
use crate::judge::{Budget, Route};
use crate::mission::Rule;
use crate::node::InstanceScope;
use crate::object::ObjectId;
use crate::placement::DirectionCone;
use crate::schedule::Span;
use crate::shape::Shape;
use crate::surface::{Approach, Occupant};
use crate::tag::TagQuery;
use cv_determinism::{math, Aabb, Vec3};

use std::collections::BTreeMap;

/// **How** collision is derived from a component's geometry.
///
/// ⚠ **This says how, not whether it is drawn.** *Visible* is a separate field, because a hologram is
/// seen and not touched and a blocking wall behind a facade is touched and not seen — a single enum
/// covering both could express neither. The design names this on `MeshResource::derive_collision(mode)`,
/// and it means the same thing on a shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CollisionMode {
    /// Nothing collides with it.
    None,
    /// The enclosing box — the cheapest broad-phase stand-in.
    Box,
    /// The convex hull. ⚠ Fills in a concavity in the **optimistic** direction, which is the softlock
    /// direction, so [`crate::CollisionBody::fit_error`] is what keeps that measurable.
    Hull,
    /// The geometry as authored.
    ///
    /// ⚠ **The default, and for a parametric shape it costs nothing** — collision is computed from the
    /// shape's parameters analytically, so *exact* is not the expensive option here. It is expensive
    /// only for an imported mesh, which is the one case a developer would step down from it.
    #[default]
    Exact,
}

impl CollisionMode {
    /// Does anything collide at all?
    pub fn collides(self) -> bool {
        self != CollisionMode::None
    }

    /// Is this an approximation that may over-report solidity?
    ///
    /// ⚠ The predicate a firm answer must check: a `Box` or `Hull` body can report *blocked* where the
    /// realized geometry is clear, so a decision resting on one is provisional until L4.
    pub fn is_conservative(self) -> bool {
        matches!(self, CollisionMode::Box | CollisionMode::Hull)
    }
}

/// Which way a traversal may be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    /// From the near side to the far side.
    Forward,
    /// From the far side back.
    Reverse,
}

/// One of the eight core behaviours.
///
/// ⚠ **A closed set in the core, an open one in content.** These are the primitives every project
/// composes from; a project's own components are authored subclasses rather than additions here.
#[derive(Clone, Debug, PartialEq)]
pub enum Component {
    /// Imported geometry, with a submesh→surface map.
    ///
    /// ⚠ **The one place tessellation is legitimate.** Every other collision shape is parametric; an
    /// imported mesh is a `MeshResource`, and its content hash is what keeps it deterministic.
    Mesh {
        asset: ObjectId,
        /// Submesh name → the `Surface` it means. A `BTreeMap` because iteration order reaches
        /// generation, and a hash map's would not be stable across runs.
        surfaces: BTreeMap<String, ObjectId>,
        collision_mode: CollisionMode,
        visible: bool,
    },
    /// A parametric primitive.
    Shape {
        shape: Shape,
        surface: Option<ObjectId>,
        collision_mode: CollisionMode,
        visible: bool,
    },
    /// An attachment socket.
    ///
    /// ⚠ `name` is **a label, not a lookup key** — two mounts may share one, and content that keyed off
    /// it would break the moment a developer renamed a socket for readability.
    Mount {
        name: String,
        /// ⚠ A **query**, not a class list: *"any torch"* must pick up a torch added next week.
        accepts: TagQuery,
        faces: Vec<crate::geometry::Face>,
        /// Space that must stay empty for something to sit here.
        clearance: CollisionBody,
    },
    /// A spatial delta that becomes a **directed** graph edge.
    ///
    /// ⚠ `admits` takes a **direction**, so one edge can be open one way and gated the other — a
    /// shortcut opened from the far side, a barred door, a one-way drop. A single boolean could
    /// express none of those.
    Traversal {
        /// Horizontal distance covered, as a range.
        run: Span,
        /// Vertical distance covered, as a range. Negative for a descent.
        rise: Span,
        direction: DirectionCone,
        /// What must hold to use it, per direction.
        forward: Rule,
        reverse: Rule,
        /// What using it costs, in world units of the budget it is measured against.
        cost: f64,
        approach: Option<Approach>,
        /// ▶ **PROPOSED.** The volume that must be empty for the move to exist.
        ///
        /// ⚠ `run` and `rise` describe the move's *endpoints*, so they admit a jump whose actual arc
        /// would not clear a low ceiling. This is the swept volume — **the solver needs the volume,
        /// not the curve** — defaulting conservatively to the box implied by `run` × `rise` and
        /// tightenable by a developer who wants a parabola.
        clearance: CollisionBody,
    },
    /// A place the world returns to a known-good state.
    ///
    /// ⚠ **P15's second satisfaction route.** Permissive rather than restrictive: it is what lets the
    /// solver take an attractive one-way transition *and* guarantee the reset.
    Checkpoint {
        restores: Vec<ObjectId>,
        restores_occupant: bool,
        scope: InstanceScope,
    },
    /// A node in a fast-travel network.
    ///
    /// ⚠ **Collapses traversal cost across the whole World.** Invisible to the solver means every
    /// difficulty judgement in the project is measured against a budget that no longer describes it.
    FastTravel {
        network: String,
        /// What arriving costs — `None` for a free jump.
        cost: Option<Budget>,
        unlocked_by: Rule,
    },
    /// Sets a world-state variable.
    ///
    /// ⚠ `while_occupied_by` is the important field: it makes **dwell** an ordinary reading of a state
    /// setter — *"while something with this component is on me"* — rather than a separate trigger kind
    /// the whole system would have to learn about.
    StateSetter {
        variable: String,
        to_value: String,
        while_occupied_by: Option<ObjectId>,
    },
    /// *"Place me **on** an edge of this kind, and close it."*
    ///
    /// ⚠ Without this a barrier can only be authored as geometry, which violates P2 by construction:
    /// deleting a region rather than gating it.
    BlocksTraversal {
        matching: ObjectId,
        route: Option<Route>,
    },
}

impl Component {
    /// A traversal with the **conservative default clearance**: the box implied by `run` × `rise`.
    ///
    /// ⚠ **Conservative by construction (P1).** The default over-reserves — a jump's real arc is a
    /// parabola inside this box, never outside it — so a developer who tightens it is making a claim,
    /// and a developer who ignores it is safe. The reverse default would let arcs clip ceilings that
    /// nobody declared.
    pub fn traversal(run: Span, rise: Span, forward: Rule, reverse: Rule) -> Component {
        let w = if run.is_bounded() { run.max() } else { 0.0 };
        let h = if rise.is_bounded() {
            math::max(rise.max().abs(), rise.min().abs())
        } else {
            0.0
        };
        let clearance = if w > 0.0 || h > 0.0 {
            CollisionBody::of(Shape::Cube {
                extents: Vec3::new(w, h, w),
                bevel: 0.0,
            })
        } else {
            CollisionBody::empty()
        };
        Component::Traversal {
            run,
            rise,
            direction: DirectionCone::any(),
            forward,
            reverse,
            cost: 0.0,
            approach: None,
            clearance,
        }
    }

    /// The hook family this component contributes to.
    pub fn name(&self) -> &'static str {
        match self {
            Component::Mesh { .. } => "MeshComponent",
            Component::Shape { .. } => "ShapeComponent",
            Component::Mount { .. } => "MountComponent",
            Component::Traversal { .. } => "TraversalComponent",
            Component::Checkpoint { .. } => "CheckpointComponent",
            Component::FastTravel { .. } => "FastTravelComponent",
            Component::StateSetter { .. } => "StateSetterComponent",
            Component::BlocksTraversal { .. } => "BlocksTraversalComponent",
        }
    }

    /// Does this component contribute collision?
    pub fn is_collidable(&self) -> bool {
        match self {
            Component::Mesh { collision_mode, .. } | Component::Shape { collision_mode, .. } => {
                collision_mode.collides()
            }
            _ => false,
        }
    }

    /// Is this component drawn?
    pub fn is_visible(&self) -> bool {
        match self {
            Component::Mesh { visible, .. } | Component::Shape { visible, .. } => *visible,
            _ => false,
        }
    }

    /// Does using this component in that direction work, for that occupant?
    ///
    /// ⚠ **Direction-aware by construction.** A shortcut opened from the far side is `Never` forward
    /// and `Always` reverse, which a single rule could not say.
    pub fn admits(&self, direction: Direction, occupant: &Occupant) -> bool {
        let Component::Traversal {
            forward, reverse, ..
        } = self
        else {
            return false;
        };
        let rule = match direction {
            Direction::Forward => forward,
            Direction::Reverse => reverse,
        };
        rule.is_satisfied(&occupant.held.iter().copied().collect())
    }
}

/// One component as attached to an Actor.
#[derive(Clone, Debug, PartialEq)]
pub struct Attached {
    /// What it is.
    pub component: Component,
    /// Disabled components contribute nothing, and are skipped by aggregation.
    pub enabled: bool,
    /// Where it sits relative to its owner.
    pub offset: Vec3,
}

impl Attached {
    /// Attach a component, enabled, at the owner's origin.
    pub fn new(component: Component) -> Self {
        Attached {
            component,
            enabled: true,
            offset: Vec3::ZERO,
        }
    }

    /// Attach it disabled.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Offset it from the owner.
    pub fn at(mut self, offset: Vec3) -> Self {
        self.offset = offset;
        self
    }
}

/// An Actor's components, in attach order.
///
/// ⚠ **Attach order is the aggregation order**, and it is stable rather than sorted: two components
/// answering the same hook do so in the order a developer attached them, which is the order they see
/// in the inspector. Sorting by anything else would make the inspector and the behaviour disagree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Components {
    attached: Vec<Attached>,
}

impl Components {
    /// No components — an Actor that answers every hook with nothing.
    pub fn new() -> Self {
        Components::default()
    }

    /// Attach one.
    pub fn with(mut self, attached: Attached) -> Self {
        self.attached.push(attached);
        self
    }

    /// Every component, enabled or not, in attach order.
    pub fn all(&self) -> &[Attached] {
        &self.attached
    }

    /// **The aggregation default**: every *enabled* component, in attach order.
    ///
    /// ⚠ Disabled components are skipped rather than removed, so toggling one in the editor does not
    /// renumber the others.
    pub fn enabled(&self) -> impl Iterator<Item = &Component> {
        self.attached
            .iter()
            .filter(|a| a.enabled)
            .map(|a| &a.component)
    }

    /// Does this Actor carry a component of that name?
    pub fn has(&self, name: &str) -> bool {
        self.enabled().any(|c| c.name() == name)
    }

    /// Everything that contributes collision.
    pub fn collidable(&self) -> impl Iterator<Item = &Component> {
        self.enabled().filter(|c| c.is_collidable())
    }

    /// The fast-travel networks this Actor belongs to.
    ///
    /// ⚠ Returned as a list because one Actor may join several — a bonfire that is both a regional
    /// waypoint and a world-network node is one Actor, not two.
    pub fn networks(&self) -> Vec<&str> {
        self.enabled()
            .filter_map(|c| match c {
                Component::FastTravel { network, .. } => Some(network.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Is this a place the world can be restored from?
    pub fn is_checkpoint(&self) -> bool {
        self.has("CheckpointComponent")
    }

    /// The collision one component contributes, offset into its owner's frame.
    ///
    /// ⚠ **A `Mesh` contributes nothing here yet, and that is a stated hole rather than a silent
    /// zero.** Its collision comes from an imported `MeshResource`, which is M14's; until then a mesh
    /// component is visible and answers no spatial question. Anything that reads this must not mistake
    /// an empty body for *"nothing is there"*.
    fn body_of(attached: &Attached) -> CollisionBody {
        match &attached.component {
            Component::Shape {
                shape,
                surface,
                collision_mode,
                ..
            } if collision_mode.collides() => {
                let mut island = CollisionData::new(shape.clone()).at(attached.offset);
                island.surface = *surface;
                CollisionBody::empty().with(island)
            }
            _ => CollisionBody::empty(),
        }
    }

    /// **The aggregation default for `collision()`** — the union of every enabled component's
    /// collision.
    ///
    /// ⚠ This is what *"forgetting one forwarding line must not make a mechanic silently do nothing"*
    /// looks like in code: attaching a component changes this answer without anyone wiring it up.
    pub fn collision(&self) -> CollisionBody {
        self.attached
            .iter()
            .filter(|a| a.enabled)
            .fold(CollisionBody::empty(), |acc, a| {
                acc.union(&Components::body_of(a))
            })
    }

    /// **The aggregation default for `footprint()`** — space reserved at skeleton time.
    ///
    /// ⚠ Same default as [`Self::collision`], and they are still **two questions**: a developer who
    /// reserves a shaft for an elevator but requires landings at both ends overrides one and not the
    /// other. Fusing them would make them inflate the footprint to fake the clearance, and worlds go
    /// sparse.
    pub fn footprint(&self) -> CollisionBody {
        self.collision()
    }

    /// **The aggregation default for `clearance()`** — space that must stay *empty*.
    ///
    /// ⚠ **Empty by default**, gathered only from the components that genuinely declare a need: a
    /// mount's socket room and a traversal's swept volume. Defaulting to the collision union would
    /// reserve a hole the size of every object around every object.
    pub fn clearance(&self) -> CollisionBody {
        self.attached
            .iter()
            .filter(|a| a.enabled)
            .fold(CollisionBody::empty(), |acc, a| match &a.component {
                Component::Mount { clearance, .. } | Component::Traversal { clearance, .. } => {
                    acc.union(clearance)
                }
                _ => acc,
            })
    }

    /// **The aggregation default for `bounds()`** — the cheapest conservative box.
    pub fn bounds(&self) -> Aabb {
        self.collision().bounds()
    }

    /// How many components answer a given hook family.
    ///
    /// ⚠ The number the editor's *"forgot a forwarding line"* lint compares against: an override that
    /// answers for fewer components than are attached is the silent-do-nothing bug.
    pub fn contributors(&self, name: &str) -> usize {
        self.enabled().filter(|c| c.name() == name).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Face;

    fn oid(n: &str) -> ObjectId {
        ObjectId::derived("actor", n)
    }
    fn unlock(n: &str) -> ObjectId {
        ObjectId::derived("unlock", n)
    }

    fn checkpoint() -> Component {
        Component::Checkpoint {
            restores: vec![oid("enemies")],
            restores_occupant: true,
            scope: InstanceScope::Area,
        }
    }

    fn fast_travel(network: &str) -> Component {
        Component::FastTravel {
            network: network.to_string(),
            cost: Some(Budget::distance(0.0)),
            unlocked_by: Rule::Always,
        }
    }

    // --- the green criterion --------------------------------------------------------------------

    #[test]
    fn a_bonfire_composes_from_two_components_with_no_special_case() {
        // ⚠ **The milestone's green criterion.** A bonfire is a checkpoint *and* a fast-travel node.
        // If composition needed a special case, the component model would be a taxonomy rather than a
        // composition — and every future combination would need core support.
        let bonfire = Components::new()
            .with(Attached::new(checkpoint()))
            .with(Attached::new(fast_travel("world")));

        assert!(bonfire.is_checkpoint());
        assert_eq!(bonfire.networks(), vec!["world"]);
        assert_eq!(bonfire.all().len(), 2);
    }

    #[test]
    fn a_bench_and_a_stag_each_carry_one_of_the_two() {
        // The other half of the criterion: the pieces work alone, so the bonfire is genuinely a
        // composition rather than a third type wearing two names.
        let bench = Components::new().with(Attached::new(checkpoint()));
        assert!(bench.is_checkpoint());
        assert!(bench.networks().is_empty(), "a bench goes nowhere");

        let stag = Components::new().with(Attached::new(fast_travel("stagways")));
        assert!(!stag.is_checkpoint(), "a stag does not restore the world");
        assert_eq!(stag.networks(), vec!["stagways"]);
    }

    #[test]
    fn one_actor_may_join_several_networks() {
        // A regional waypoint that is also a world node is one Actor, not two.
        let hub = Components::new()
            .with(Attached::new(fast_travel("regional")))
            .with(Attached::new(fast_travel("world")));
        assert_eq!(hub.networks(), vec!["regional", "world"]);
    }

    // --- aggregation ------------------------------------------------------------------------------

    #[test]
    fn aggregation_follows_attach_order_and_not_some_other_order() {
        // ⚠ Attach order is what the inspector shows. Sorting by anything else would make the
        // inspector and the behaviour disagree, which is a bug nobody can see.
        let c = Components::new()
            .with(Attached::new(fast_travel("a")))
            .with(Attached::new(checkpoint()))
            .with(Attached::new(fast_travel("b")));
        let names: Vec<&str> = c.enabled().map(Component::name).collect();
        assert_eq!(
            names,
            vec![
                "FastTravelComponent",
                "CheckpointComponent",
                "FastTravelComponent"
            ]
        );
    }

    #[test]
    fn a_disabled_component_contributes_nothing_but_keeps_its_place() {
        let c = Components::new()
            .with(Attached::new(checkpoint()).disabled())
            .with(Attached::new(fast_travel("world")));
        assert!(!c.is_checkpoint(), "disabled means it does not answer");
        assert_eq!(c.all().len(), 2, "but it is still attached");
        assert_eq!(c.enabled().count(), 1);
    }

    #[test]
    fn contributors_counts_what_an_override_would_have_to_answer_for() {
        // ⚠ The number the editor's lint compares against. An override answering for fewer components
        // than are attached is the silent-do-nothing bug the aggregation default exists to prevent.
        let c = Components::new()
            .with(Attached::new(fast_travel("a")))
            .with(Attached::new(fast_travel("b")))
            .with(Attached::new(checkpoint()));
        assert_eq!(c.contributors("FastTravelComponent"), 2);
        assert_eq!(c.contributors("CheckpointComponent"), 1);
        assert_eq!(c.contributors("MountComponent"), 0);
    }

    // --- direction --------------------------------------------------------------------------------

    #[test]
    fn one_edge_can_be_open_one_way_and_gated_the_other() {
        // ⚠ A shortcut opened from the far side — the shape a single boolean could not express.
        let key = unlock("vault_key");
        let door = Component::traversal(
            Span::exactly(2.0),
            Span::exactly(0.0),
            Rule::has(key),
            Rule::Always,
        );

        let empty = Occupant::player([]);
        let keyed = Occupant::player([key]);

        assert!(
            !door.admits(Direction::Forward, &empty),
            "barred from outside"
        );
        assert!(door.admits(Direction::Reverse, &empty), "opens from inside");
        assert!(door.admits(Direction::Forward, &keyed), "the key works");
    }

    #[test]
    fn a_one_way_drop_is_never_reversible() {
        let drop = Component::traversal(
            Span::exactly(0.0),
            Span::exactly(-6.0),
            Rule::Always,
            Rule::Never,
        );
        let anyone = Occupant::player([unlock("everything")]);
        assert!(drop.admits(Direction::Forward, &anyone));
        assert!(
            !drop.admits(Direction::Reverse, &anyone),
            "no amount of held content climbs a drop"
        );
    }

    #[test]
    fn only_a_traversal_admits_anything() {
        // Asking a mount whether it admits a direction is a category error, and the answer is no.
        let mount = Component::Mount {
            name: "socket".into(),
            accepts: TagQuery::inherited("Prop.Torch"),
            faces: vec![Face::PosY],
            clearance: CollisionBody::empty(),
        };
        assert!(!mount.admits(Direction::Forward, &Occupant::player([])));
    }

    // --- visible versus collidable ----------------------------------------------------------------

    #[test]
    fn a_conservative_mode_is_marked_as_one() {
        // ⚠ A `Box` or `Hull` body may report *blocked* where the realized geometry is clear — the
        // optimistic-solidity direction. A decision resting on one is provisional until L4, and that is
        // only checkable because the mode says so.
        assert!(CollisionMode::Hull.is_conservative());
        assert!(CollisionMode::Box.is_conservative());
        assert!(!CollisionMode::Exact.is_conservative());
        assert!(!CollisionMode::None.is_conservative());
        assert!(!CollisionMode::None.collides());
    }

    #[test]
    fn exact_is_the_default_because_a_parametric_shape_costs_nothing_to_be_exact_about() {
        // ⚠ Collision is computed from the shape's parameters, so *exact* is not the expensive option
        // for a `ShapeComponent`. Defaulting to `Hull` would approximate something that needed no
        // approximating, in the softlock direction, for free.
        assert_eq!(CollisionMode::default(), CollisionMode::Exact);
    }

    #[test]
    fn visible_and_collidable_are_independent() {
        // ⚠ A hologram is seen and not touched; a blocking wall behind a facade is touched and not
        // seen. A model that conflated the two could express neither.
        let hologram = Component::Shape {
            shape: Shape::Cube {
                extents: Vec3::new(2.0, 2.0, 2.0),
                bevel: 0.0,
            },
            surface: None,
            collision_mode: CollisionMode::None,
            visible: true,
        };
        let blocker = Component::Shape {
            shape: Shape::Cube {
                extents: Vec3::new(2.0, 2.0, 2.0),
                bevel: 0.0,
            },
            surface: None,
            collision_mode: CollisionMode::Exact,
            visible: false,
        };
        assert!(hologram.is_visible() && !hologram.is_collidable());
        assert!(!blocker.is_visible() && blocker.is_collidable());
    }

    // --- the two easy-to-miss ones ------------------------------------------------------------------

    #[test]
    fn a_checkpoint_declares_what_comes_back_and_it_is_not_unlocks() {
        // ⚠ Unlocks are monotone and can never be lost, so restoring one is meaningless. What comes
        // back is placed content.
        let Component::Checkpoint {
            restores,
            restores_occupant,
            ..
        } = checkpoint()
        else {
            panic!("expected a checkpoint");
        };
        assert_eq!(restores, vec![oid("enemies")]);
        assert!(restores_occupant);
    }

    #[test]
    fn dwell_is_an_ordinary_reading_of_a_state_setter() {
        // ⚠ `while_occupied_by` is what keeps "stand here for three seconds" from needing its own
        // trigger kind that the whole system would have to learn about.
        let plate = Component::StateSetter {
            variable: "gate".into(),
            to_value: "open".into(),
            while_occupied_by: Some(oid("WeightComponent")),
        };
        let Component::StateSetter {
            while_occupied_by, ..
        } = plate
        else {
            panic!("expected a state setter");
        };
        assert_eq!(while_occupied_by, Some(oid("WeightComponent")));
    }

    #[test]
    fn a_barrier_is_placed_on_an_edge_rather_than_deleting_a_region() {
        // ⚠ P2 — gate a region, never delete it. Without this component a barrier could only be
        // authored as geometry, which removes the region instead of closing it.
        let gate = Component::BlocksTraversal {
            matching: oid("TetherComponent"),
            route: None,
        };
        assert_eq!(gate.name(), "BlocksTraversalComponent");
    }

    #[test]
    fn all_eight_are_present_and_name_themselves() {
        let names: Vec<&str> = vec![
            Component::Mesh {
                asset: oid("m"),
                surfaces: BTreeMap::new(),
                collision_mode: CollisionMode::Exact,
                visible: true,
            },
            Component::Shape {
                shape: Shape::Sphere { radius: 1.0 },
                surface: None,
                collision_mode: CollisionMode::Exact,
                visible: true,
            },
            Component::Mount {
                name: "s".into(),
                accepts: TagQuery::exact("Prop"),
                faces: vec![],
                clearance: CollisionBody::empty(),
            },
            Component::traversal(
                Span::exactly(1.0),
                Span::exactly(0.0),
                Rule::Always,
                Rule::Always,
            ),
            checkpoint(),
            fast_travel("w"),
            Component::StateSetter {
                variable: "v".into(),
                to_value: "x".into(),
                while_occupied_by: None,
            },
            Component::BlocksTraversal {
                matching: oid("T"),
                route: None,
            },
        ]
        .iter()
        .map(Component::name)
        .collect();
        assert_eq!(names.len(), 8);
        assert_eq!(
            names
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            8,
            "each names itself distinctly"
        );
    }
}
