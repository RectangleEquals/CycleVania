//! Hand-written mechanics that stand in for CVScript until the VM exists.
//!
//! # Why these live in the library rather than in tests
//!
//! They are not test scaffolding; they are the **first implementations of the mechanic interface**,
//! and the pipeline is built against them for milestones M08–M14. Presets and the CLI need them too,
//! not just `#[cfg(test)]` code. At M18 each is replaced by the equivalent `.cvs` script compiled to
//! bytecode, and — the claim the whole seam rests on — *nothing calling them changes*.
//!
//! # They are deliberately real, not stubs
//!
//! A stub returning `None` everywhere would let the pipeline compile without proving anything. These
//! implement behaviour with actual consequences: [`Barrier`] gates a traversal on holding a capability,
//! so the solver has something to reason about; [`Glass`] passes some flows and blocks others, which
//! is the TC16 case that motivated per-flow surfaces. If the interface were shaped wrong, writing
//! these is where it would show.
//!
//! Each carries a doc note naming the CVScript it will become, so the eventual translation is a
//! transcription rather than a redesign.

use crate::content::ContentKind;
use crate::context::Context;
use crate::mechanic::{
    Constraint, Constraints, FlowKind, Mechanic, Request, Traversal, TraversalKind, Volume,
};
use crate::node::NodeKind;
use crate::object::ObjectId;
use cv_determinism::{Aabb, Vec3};

/// A door: occupies a doorway, and affords passage only to whoever holds its key.
///
/// Becomes:
/// ```gdscript
/// class Door extends Actor:
///     exposed var key: Item
///     api func footprint(ctx) -> Volume:   return Volume.cube(2.0)
///     api func affords(ctx) -> Array[Traversal]:  return [Traversal.gated(Walk, [key])]
///     api func constraints(ctx) -> Constraints:   return Constraints.within(Space)
/// ```
#[derive(Debug, Clone)]
pub struct Door {
    /// What opens it. `None` makes it a plain opening rather than a gate.
    pub key: Option<ObjectId>,
    /// How wide a doorway it needs.
    pub width: f64,
}

impl Door {
    /// A door gated on an item or capability.
    pub fn locked_by(key: ObjectId) -> Self {
        Door {
            key: Some(key),
            width: 2.0,
        }
    }

    /// An ungated opening.
    pub fn open() -> Self {
        Door {
            key: None,
            width: 2.0,
        }
    }
}

impl Mechanic for Door {
    fn kind(&self) -> ContentKind {
        ContentKind::Actor
    }

    fn label(&self) -> &str {
        "Door"
    }

    fn footprint(&self, _ctx: &Context<'_>) -> Option<Volume> {
        Some(Volume::with_clearance(
            Aabb::from_center_extents(Vec3::ZERO, Vec3::new(self.width * 0.5, 0.25, 1.5)),
            0.5,
        ))
    }

    fn constraints(&self, _ctx: &Context<'_>) -> Constraints {
        let base = Constraints::none().and(Constraint::WithinScopeKind(NodeKind::Space));
        match self.key {
            // The gate's own reachability depends on its key — the fact L2 needs to place them in a
            // solvable order rather than discovering the cycle later.
            Some(key) => base.and(Constraint::RequiresCapability(key)),
            None => base,
        }
    }

    fn affords(&self, _ctx: &Context<'_>) -> Vec<Traversal> {
        vec![match self.key {
            Some(key) => Traversal::gated(TraversalKind::Walk, [key]),
            None => Traversal::open(TraversalKind::Walk),
        }]
    }

    fn request(&self, ctx: &mut Context<'_>) {
        // A preference, not a rule: doors read better spread out, but the solver may overrule.
        ctx.request(Request::PreferSpacing(6.0));
    }
}

/// A one-way drop: passable downward, impossible to climb back.
///
/// The simplest thing that can strand a player, which makes it the smallest useful test of the
/// un-softlockable pass (M10): every reachable state behind it must still reach the goal.
///
/// Becomes:
/// ```gdscript
/// class Ledge extends Actor:
///     api func affords(ctx) -> Array[Traversal]:  return [Traversal.open(Jump).one_way()]
/// ```
#[derive(Debug, Clone, Default)]
pub struct Ledge;

impl Mechanic for Ledge {
    fn kind(&self) -> ContentKind {
        ContentKind::Actor
    }

    fn label(&self) -> &str {
        "Ledge"
    }

    fn affords(&self, _ctx: &Context<'_>) -> Vec<Traversal> {
        vec![Traversal::open(TraversalKind::Jump).one_way()]
    }

    fn constraints(&self, _ctx: &Context<'_>) -> Constraints {
        Constraints::none().and(Constraint::WithinScopeKind(NodeKind::Space))
    }
}

/// A capability the player can be granted, which unlocks a kind of movement.
///
/// Becomes:
/// ```gdscript
/// class BlinkDash extends Capability:
///     api func affords(ctx) -> Array[Traversal]:  return [Traversal.open(Blink)]
/// ```
#[derive(Debug, Clone)]
pub struct MovementCapability {
    /// The movement it enables.
    pub traversal: TraversalKind,
    /// Its display name.
    pub name: String,
}

impl MovementCapability {
    /// A capability granting a movement kind.
    pub fn new(name: impl Into<String>, traversal: TraversalKind) -> Self {
        MovementCapability {
            traversal,
            name: name.into(),
        }
    }
}

impl Mechanic for MovementCapability {
    fn kind(&self) -> ContentKind {
        ContentKind::Capability
    }

    fn label(&self) -> &str {
        &self.name
    }

    fn affords(&self, _ctx: &Context<'_>) -> Vec<Traversal> {
        vec![Traversal::open(self.traversal)]
    }
}

/// A pickup that grants a capability when obtained.
///
/// Becomes:
/// ```gdscript
/// class KeyItem extends Item:
///     exposed var grants: Capability
/// ```
#[derive(Debug, Clone)]
pub struct KeyItem {
    /// The capability obtaining this confers.
    pub grants: ObjectId,
}

impl KeyItem {
    /// An item granting a capability.
    pub fn granting(capability: ObjectId) -> Self {
        KeyItem { grants: capability }
    }
}

impl Mechanic for KeyItem {
    fn kind(&self) -> ContentKind {
        ContentKind::Item
    }

    fn label(&self) -> &str {
        "KeyItem"
    }

    fn grants(&self, _ctx: &Context<'_>) -> Option<ObjectId> {
        Some(self.grants)
    }

    fn footprint(&self, _ctx: &Context<'_>) -> Option<Volume> {
        Some(Volume::cube(0.5))
    }

    fn constraints(&self, _ctx: &Context<'_>) -> Constraints {
        Constraints::none().and(Constraint::WithinScopeKind(NodeKind::Space))
    }
}

/// **Glass** — the surface that motivated per-flow interaction.
///
/// It blocks bodies and bullets but passes light and sight, so "is it solid?" has no single answer.
/// A surface is solid *to something*, and which flows it stops is the mechanic (TC16).
///
/// Becomes:
/// ```gdscript
/// class Glass extends SurfaceProperty:
///     api func blocks(ctx, flow) -> bool:
///         return flow == FlowKind.Ballistic or flow == FlowKind.Walking
/// ```
#[derive(Debug, Clone, Default)]
pub struct Glass;

impl Mechanic for Glass {
    fn kind(&self) -> ContentKind {
        ContentKind::SurfaceProperty
    }

    fn label(&self) -> &str {
        "Glass"
    }

    fn blocks(&self, _ctx: &Context<'_>, flow: FlowKind) -> bool {
        matches!(
            flow,
            FlowKind::Ballistic | FlowKind::Walking | FlowKind::Portal
        )
    }
}

/// A mirror: passes nothing, but sends light back.
///
/// The counterpart to [`Glass`] — where glass is selectively transparent, this is selectively
/// *reflective*, and together they cover both halves of what a surface can do to a flow.
///
/// Becomes:
/// ```gdscript
/// class Deflective extends SurfaceProperty:
///     api func redirects(ctx, flow, incoming) -> Vec3?:
///         return ctx.reflect(incoming, self.normal) if flow == FlowKind.Laser else null
/// ```
#[derive(Debug, Clone)]
pub struct Deflective {
    /// The surface normal to reflect about.
    pub normal: Vec3,
}

impl Deflective {
    /// A mirror facing `normal`.
    pub fn facing(normal: Vec3) -> Self {
        Deflective {
            normal: normal.normalized(),
        }
    }
}

impl Default for Deflective {
    fn default() -> Self {
        Deflective::facing(Vec3::Z)
    }
}

impl Mechanic for Deflective {
    fn kind(&self) -> ContentKind {
        ContentKind::SurfaceProperty
    }

    fn label(&self) -> &str {
        "Deflective"
    }

    fn blocks(&self, _ctx: &Context<'_>, flow: FlowKind) -> bool {
        // Solid to everything physical; light is redirected rather than stopped.
        !matches!(flow, FlowKind::Laser | FlowKind::Sight)
    }

    fn redirects(&self, _ctx: &Context<'_>, flow: FlowKind, incoming: Vec3) -> Option<Vec3> {
        match flow {
            // M11 routes this through `ctx.reflect`; the arithmetic is identical either way.
            FlowKind::Laser | FlowKind::Sight => Some(incoming.reflect(self.normal)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mechanic::MechanicRegistry;

    #[test]
    fn a_locked_door_gates_its_traversal_and_declares_the_dependency() {
        let key = ObjectId::derived("item", "key_bronze");
        let door = Door::locked_by(key);
        let ctx = Context::detached();

        let afforded = door.affords(&ctx);
        assert_eq!(afforded.len(), 1);
        assert_eq!(afforded[0].requires, vec![key], "passage costs the key");
        assert!(afforded[0].reversible, "a door opens both ways");

        // The same dependency shows up as a constraint, so L2 can order placement before it routes.
        assert_eq!(
            door.constraints(&ctx)
                .required_capabilities()
                .collect::<Vec<_>>(),
            vec![key]
        );

        // An open doorway gates nothing.
        let open = Door::open();
        assert!(open.affords(&ctx)[0].requires.is_empty());
        assert!(open
            .constraints(&ctx)
            .required_capabilities()
            .next()
            .is_none());
    }

    #[test]
    fn a_door_asks_for_spacing_without_demanding_it() {
        let mut ctx = Context::detached();
        Door::open().request(&mut ctx);
        assert_eq!(ctx.requests(), &[Request::PreferSpacing(6.0)]);
        // It is a request, not a constraint — the solver may overrule it.
        assert!(!Door::open()
            .constraints(&Context::detached())
            .iter()
            .any(|c| matches!(c, Constraint::MinClearance(_))));
    }

    #[test]
    fn a_ledge_is_a_one_way_commit() {
        let t = Ledge.affords(&Context::detached());
        assert!(
            !t[0].reversible,
            "the smallest thing that can strand a player"
        );
        assert_eq!(t[0].kind, TraversalKind::Jump);
    }

    #[test]
    fn an_item_grants_a_capability_that_affords_movement() {
        let ctx = Context::detached();
        let dash = ObjectId::derived("capability", "blink_dash");
        assert_eq!(KeyItem::granting(dash).grants(&ctx), Some(dash));

        // And the capability itself is what turns into a traversal edge.
        let cap = MovementCapability::new("Blink Dash", TraversalKind::Blink);
        assert_eq!(cap.affords(&ctx)[0].kind, TraversalKind::Blink);
        assert_eq!(cap.kind(), ContentKind::Capability);
    }

    #[test]
    fn glass_is_solid_to_some_flows_and_not_others() {
        // The TC16 case: one surface, opposite answers depending on what is asking.
        let ctx = Context::detached();
        assert!(Glass.blocks(&ctx, FlowKind::Walking));
        assert!(Glass.blocks(&ctx, FlowKind::Ballistic));
        assert!(!Glass.blocks(&ctx, FlowKind::Laser));
        assert!(!Glass.blocks(&ctx, FlowKind::Sight));
        // And it redirects nothing — it is transparent, not reflective.
        assert!(Glass.redirects(&ctx, FlowKind::Laser, Vec3::X).is_none());
    }

    #[test]
    fn a_mirror_reflects_light_and_stops_everything_else() {
        let ctx = Context::detached();
        let mirror = Deflective::facing(Vec3::Z);
        let down = Vec3::new(0.0, 0.0, -1.0);
        let bounced = mirror.redirects(&ctx, FlowKind::Laser, down).unwrap();
        assert!(
            bounced.approx_eq(Vec3::new(0.0, 0.0, 1.0), 1e-12),
            "straight down bounces back up"
        );
        assert!(mirror.blocks(&ctx, FlowKind::Walking));
        assert!(
            !mirror.blocks(&ctx, FlowKind::Laser),
            "light is redirected, not stopped"
        );
        assert!(mirror.redirects(&ctx, FlowKind::Walking, down).is_none());
    }

    #[test]
    fn fixtures_slot_into_the_registry_and_answer_uniformly() {
        // The pipeline's view: one lookup, one call, no knowledge of which fixture it got.
        let key = ObjectId::derived("item", "key_bronze");
        let door_id = ObjectId::derived("actor", "door");
        let glass_id = ObjectId::derived("surface", "glass");

        let mut reg = MechanicRegistry::new();
        reg.register(door_id, Box::new(Door::locked_by(key)));
        reg.register(glass_id, Box::new(Glass));

        let ctx = Context::detached();
        assert_eq!(reg.get(door_id).label(), "Door");
        assert!(reg.get(door_id).footprint(&ctx).is_some());
        // A surface has no footprint — the default applies, without the caller checking the kind.
        assert!(reg.get(glass_id).footprint(&ctx).is_none());
        assert!(reg.get(glass_id).blocks(&ctx, FlowKind::Walking));
        assert!(!reg.get(door_id).blocks(&ctx, FlowKind::Walking));
    }
}
