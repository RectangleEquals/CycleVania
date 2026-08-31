//! The **mechanic interface** — the seam between the algorithm and the behaviour it reasons about.
//!
//! # The cycle this exists to break
//!
//! The core's API shape depends on what mechanics need to express. CVScript is the ergonomic wrapper
//! over that API, so the API has to exist first. But the pipeline cannot be built without *some*
//! mechanics to call. That is a genuine circular dependency, and the way out is this trait:
//!
//! > [`Mechanic`] is the **exact shape** CVScript's overridable `api` methods will have. Today it is
//! > implemented by hand-written Rust ([`crate::fixtures`]); at M18 it is implemented by the bytecode
//! > VM instead. **The pipeline does not change.**
//!
//! That last sentence is the whole point, and it is a testable claim rather than an aspiration — see
//! `tests/mechanic_seam.rs`, which swaps a Rust fixture for a VM-shaped stand-in and asserts the
//! calling code is untouched. If the pipeline ever has to know *which* kind of mechanic it is calling,
//! the seam has failed and the cycle is back.
//!
//! # Why one uniform trait, when the class hierarchy is not uniform
//!
//! A `SurfaceProperty` has no footprint; an `Actor` does not redirect a laser. So it may look wrong
//! that both implement the same trait. The split is deliberate and falls on a real boundary:
//!
//! * **The core dispatches uniformly.** At M18 the VM will invoke "method N on this instance"; making
//!   the core-side surface uniform is what lets that dispatch be one mechanism rather than several.
//!   Callbacks a mechanic does not care about keep their default, exactly as the design specifies
//!   ("defaults return no footprint / no constraints / none").
//! * **The *language* narrows it.** M16's api-signature checker rejects a `footprint` on a
//!   `SurfaceProperty` subclass, with a fuzzy-match hint. A dev never sees the uniform surface; they
//!   see the methods their class actually has.
//!
//! So the narrowing lives where it can produce a good error message, and the uniformity lives where it
//! makes dispatch simple. Neither is compromised.

use crate::content::ContentKind;
use crate::context::Context;
use crate::node::NodeKind;
use crate::object::ObjectId;
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use cv_determinism::{Aabb, Vec3};
use std::collections::BTreeMap;
use std::fmt;

// ---------------------------------------------------------------------------------------------
// Volume — what a mechanic asks the world to reserve
// ---------------------------------------------------------------------------------------------

/// The space a mechanic needs.
///
/// Coarse for now — bounds plus a clearance margin — because L4 is where volumes become real (M14
/// gives them hull geometry). The **type in the signature is stable**; only its internals grow, so
/// `footprint(ctx) -> Volume` does not change shape when that happens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    bounds: Aabb,
    /// Free space required around the bounds, in world units.
    clearance: f64,
}

impl Volume {
    /// A volume occupying `bounds` with no clearance requirement.
    pub fn new(bounds: Aabb) -> Self {
        Volume {
            bounds,
            clearance: 0.0,
        }
    }

    /// A volume requiring `clearance` free space around it.
    pub fn with_clearance(bounds: Aabb, clearance: f64) -> Self {
        Volume {
            bounds,
            clearance: clearance.max(0.0),
        }
    }

    /// A cube of `size` centred on the origin.
    pub fn cube(size: f64) -> Self {
        Volume::new(Aabb::from_center_extents(
            Vec3::ZERO,
            Vec3::splat(size * 0.5),
        ))
    }

    /// The occupied bounds, excluding clearance.
    pub fn bounds(&self) -> Aabb {
        self.bounds
    }

    /// The required free margin.
    pub fn clearance(&self) -> f64 {
        self.clearance
    }

    /// Bounds plus clearance — the space that must actually be free.
    pub fn required_bounds(&self) -> Aabb {
        self.bounds.expanded(self.clearance)
    }

    /// Does this fit inside `envelope`, clearance included?
    pub fn fits_within(&self, envelope: Aabb) -> bool {
        envelope.contains(&self.required_bounds())
    }
}

// ---------------------------------------------------------------------------------------------
// Constraints — hard placement predicates
// ---------------------------------------------------------------------------------------------

/// A hard requirement on where a mechanic may be placed. Violating one is not an option the solver
/// may trade away — contrast [`Request`], which is negotiable.
#[derive(Clone, Debug, PartialEq)]
pub enum Constraint {
    /// The player must hold this token to reach the placement.
    RequiresToken(ObjectId),
    /// At least this much free space around the placement.
    MinClearance(f64),
    /// Only inside this kind of scope.
    WithinScopeKind(NodeKind),
    /// Keep at least `min_distance` from other instances of `content`.
    AwayFrom {
        content: ObjectId,
        min_distance: f64,
    },
    /// At most this many per scope.
    MaxPerScope(u32),
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constraint::RequiresToken(id) => write!(f, "requires token {id}"),
            Constraint::MinClearance(c) => write!(f, "needs {c} clearance"),
            Constraint::WithinScopeKind(k) => write!(f, "must sit in a {k}"),
            Constraint::AwayFrom {
                content,
                min_distance,
            } => {
                write!(f, "at least {min_distance} from {content}")
            }
            Constraint::MaxPerScope(n) => write!(f, "at most {n} per scope"),
        }
    }
}

/// A mechanic's hard placement requirements.
///
/// ▶ **GAP:** this is a deliberately small, concrete starting set. The full `Rule`/`Constraints`
/// grammar is designed at M09, where the solver can say what it actually needs to reason about — which
/// is the right time, because a grammar invented before its consumer tends to fit nothing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Constraints(Vec<Constraint>);

impl Constraints {
    /// No requirements — the default for content that can go anywhere.
    pub fn none() -> Self {
        Constraints(Vec::new())
    }

    /// Build from a list.
    pub fn of(constraints: impl IntoIterator<Item = Constraint>) -> Self {
        Constraints(constraints.into_iter().collect())
    }

    /// Add one, builder-style.
    pub fn and(mut self, c: Constraint) -> Self {
        self.0.push(c);
        self
    }

    /// The constraints, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = &Constraint> + '_ {
        self.0.iter()
    }

    /// How many.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Are there none?
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every token this placement depends on — what L2 needs to reason about accessibility.
    pub fn required_tokens(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.0.iter().filter_map(|c| match c {
            Constraint::RequiresToken(id) => Some(*id),
            _ => None,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Traversal — the movement edges a mechanic creates
// ---------------------------------------------------------------------------------------------

/// How something moves.
///
/// Host-extensible via [`TraversalKind::Custom`]: a game with a grapple or a wall-run adds its own
/// without the core needing to know what those mean.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TraversalKind {
    /// Ordinary walking.
    Walk,
    /// A jump — bounded by the host's jump arc.
    Jump,
    /// Climbing a surface.
    Climb,
    /// Through water.
    Swim,
    /// An instantaneous relocation.
    Blink,
    /// Through a portal pair.
    Portal,
    /// Host-defined.
    Custom(u32),
}

/// A movement edge a mechanic makes possible.
///
/// The mechanic declares *what kind of movement it enables and what that costs*; the solver decides
/// **where** the edge goes. That split is what lets one door definition serve every door in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct Traversal {
    /// The movement this enables.
    pub kind: TraversalKind,
    /// Tokens or items the player must hold to use it.
    pub requires: Vec<ObjectId>,
    /// Can it be traversed back the other way?
    ///
    /// `false` makes this a **one-way commit**, which the un-softlockable pass (M10) must account for:
    /// a drop you cannot climb back up strands everything behind it unless a recovery route exists.
    pub reversible: bool,
}

impl Traversal {
    /// A freely reversible traversal with no requirements.
    pub fn open(kind: TraversalKind) -> Self {
        Traversal {
            kind,
            requires: Vec::new(),
            reversible: true,
        }
    }

    /// A traversal gated on holding something.
    pub fn gated(kind: TraversalKind, requires: impl IntoIterator<Item = ObjectId>) -> Self {
        Traversal {
            kind,
            requires: requires.into_iter().collect(),
            reversible: true,
        }
    }

    /// Mark this traversal one-way.
    pub fn one_way(mut self) -> Self {
        self.reversible = false;
        self
    }
}

// ---------------------------------------------------------------------------------------------
// FlowKind — what a surface is being asked about
// ---------------------------------------------------------------------------------------------

/// A directed interaction a surface may block or redirect.
///
/// The TC16 case that motivated this: **glass** blocks walking and ballistics but passes laser and
/// sight. A surface is not simply "solid" — it is solid *to something*, and which flows it stops is
/// the whole mechanic.
///
/// Host-extensible via [`FlowKind::Custom`], since a flow is just a named directed interaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlowKind {
    /// A body moving along the ground.
    Walking,
    /// Line of sight.
    Sight,
    /// A light beam.
    Laser,
    /// A projectile.
    Ballistic,
    /// A carrying stream (Portal's funnels).
    Funnel,
    /// A portal placement.
    Portal,
    /// Host-defined.
    Custom(u32),
}

impl FlowKind {
    /// The core set, excluding host-defined flows.
    pub const CORE: [FlowKind; 6] = [
        FlowKind::Walking,
        FlowKind::Sight,
        FlowKind::Laser,
        FlowKind::Ballistic,
        FlowKind::Funnel,
        FlowKind::Portal,
    ];
}

// ---------------------------------------------------------------------------------------------
// Request — negotiable preferences
// ---------------------------------------------------------------------------------------------

/// A preference a mechanic asks for and the algorithm may grant, adapt, or deny.
///
/// The distinction from [`Constraint`] is authority. A constraint is a fact the solver must respect; a
/// request is a wish it weighs. Scripts never get setters on committed state — they ask, and the core
/// keeps final say along with the solvability guarantee.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Prefer to be placed in this kind of scope.
    PreferScopeKind(NodeKind),
    /// Prefer to be near other instances of this content.
    PreferNear(ObjectId),
    /// Prefer at least this much spacing from peers.
    PreferSpacing(f64),
    /// Prefer this many per scope, if the schedule allows.
    PreferCount(u32),
}

// ---------------------------------------------------------------------------------------------
// The trait
// ---------------------------------------------------------------------------------------------

/// **The overridable surface.** Every method here corresponds one-to-one with an `api` method
/// CVScript will expose, and every one has a default so a mechanic implements only what it cares about.
///
/// Implementors are stateless: a mechanic describes a *kind of thing*, and the same instance answers
/// for every placement of it. All the per-placement variation arrives through [`Context`].
///
/// # Why `&self` and not `&mut self`
///
/// These are queries about behaviour, invoked repeatedly during search and **re-invoked after
/// backtracking**. A mechanic that accumulated state across calls would drift out of step with a world
/// the solver had since revised — the coherence failure described in `01-core/pipeline.md`. Taking
/// `&self` makes that shape unwriteable rather than merely discouraged.
pub trait Mechanic: Send + Sync {
    /// What kind of content this is. The only non-defaulted method: a mechanic must say what it is.
    fn kind(&self) -> ContentKind;

    /// A human-facing label for traces and diagnostics.
    fn label(&self) -> &str {
        "mechanic"
    }

    // --- placement (L3) ---

    /// The space this needs reserved. `None` means it occupies nothing of its own.
    fn footprint(&self, _ctx: &Context<'_>) -> Option<Volume> {
        None
    }

    /// Negotiable placement preferences. Push them via [`Context::request`].
    fn request(&self, _ctx: &mut Context<'_>) {}

    // --- logic (L2/L3) ---

    /// Hard placement requirements.
    fn constraints(&self, _ctx: &Context<'_>) -> Constraints {
        Constraints::none()
    }

    /// Movement edges this creates. The solver decides where they attach.
    fn affords(&self, _ctx: &Context<'_>) -> Vec<Traversal> {
        Vec::new()
    }

    /// What this grants when obtained — a token id, for an `Item`.
    fn grants(&self, _ctx: &Context<'_>) -> Option<ObjectId> {
        None
    }

    // --- surfaces (L5, exercised from M11's primitives) ---

    /// Does this stop the given flow?
    fn blocks(&self, _ctx: &Context<'_>, _flow: FlowKind) -> bool {
        false
    }

    /// Does this deflect the given flow, and where to?
    ///
    /// `incoming` is the flow's direction; returning `Some(dir)` sends it off along `dir`.
    fn redirects(&self, _ctx: &Context<'_>, _flow: FlowKind, _incoming: Vec3) -> Option<Vec3> {
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------------------------

/// Maps registered content to the behaviour implementing it.
///
/// Deliberately separate from [`ContentRegistry`](crate::ContentRegistry): that one records *what
/// exists* (and is a fingerprint input), while this records *how it behaves*. At M18 the values here
/// become VM-backed instead of hand-written, and nothing else moves.
#[derive(Default)]
pub struct MechanicRegistry {
    mechanics: BTreeMap<ObjectId, Box<dyn Mechanic>>,
    /// Answers for content with no registered behaviour, so callers never branch on absence.
    fallback: DefaultMechanic,
}

impl MechanicRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        MechanicRegistry::default()
    }

    /// Attach behaviour to a content id. Replaces any previous entry.
    pub fn register(&mut self, content: ObjectId, mechanic: Box<dyn Mechanic>) -> &mut Self {
        self.mechanics.insert(content, mechanic);
        self
    }

    /// The behaviour for a content id.
    ///
    /// Never fails: unregistered content gets a do-nothing mechanic whose defaults are exactly the
    /// core's ("no footprint, no constraints, none"). The pipeline therefore has no "does this have
    /// behaviour?" branch, which is one less thing to get wrong per call site.
    pub fn get(&self, content: ObjectId) -> &dyn Mechanic {
        match self.mechanics.get(&content) {
            Some(m) => m.as_ref(),
            None => &self.fallback,
        }
    }

    /// Is behaviour registered for this content?
    pub fn contains(&self, content: ObjectId) -> bool {
        self.mechanics.contains_key(&content)
    }

    /// How many mechanics are registered.
    pub fn len(&self) -> usize {
        self.mechanics.len()
    }

    /// Is nothing registered?
    pub fn is_empty(&self) -> bool {
        self.mechanics.is_empty()
    }

    /// Every registered content id, in id order — deterministic.
    pub fn ids(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.mechanics.keys().copied()
    }
}

impl fmt::Debug for MechanicRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MechanicRegistry")
            .field("len", &self.mechanics.len())
            .finish()
    }
}

/// The behaviour of content that has none — every callback at its default.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultMechanic;

impl Mechanic for DefaultMechanic {
    fn kind(&self) -> ContentKind {
        ContentKind::Actor
    }
    fn label(&self) -> &str {
        "<default>"
    }
}

// ---------------------------------------------------------------------------------------------
// Serialization (for the descriptor, traces, and the probe)
// ---------------------------------------------------------------------------------------------

impl Serialize for TraversalKind {
    fn serialize(&self, w: &mut Writer) {
        match self {
            TraversalKind::Walk => w.u8(0),
            TraversalKind::Jump => w.u8(1),
            TraversalKind::Climb => w.u8(2),
            TraversalKind::Swim => w.u8(3),
            TraversalKind::Blink => w.u8(4),
            TraversalKind::Portal => w.u8(5),
            TraversalKind::Custom(v) => {
                w.u8(255);
                w.u32(*v);
            }
        }
    }
}

impl Deserialize for TraversalKind {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(match r.u8()? {
            0 => TraversalKind::Walk,
            1 => TraversalKind::Jump,
            2 => TraversalKind::Climb,
            3 => TraversalKind::Swim,
            4 => TraversalKind::Blink,
            5 => TraversalKind::Portal,
            255 => TraversalKind::Custom(r.u32()?),
            _ => return Err(SerError::InvalidValue("unknown TraversalKind tag")),
        })
    }
}

impl Serialize for FlowKind {
    fn serialize(&self, w: &mut Writer) {
        match self {
            FlowKind::Walking => w.u8(0),
            FlowKind::Sight => w.u8(1),
            FlowKind::Laser => w.u8(2),
            FlowKind::Ballistic => w.u8(3),
            FlowKind::Funnel => w.u8(4),
            FlowKind::Portal => w.u8(5),
            FlowKind::Custom(v) => {
                w.u8(255);
                w.u32(*v);
            }
        }
    }
}

impl Deserialize for FlowKind {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(match r.u8()? {
            0 => FlowKind::Walking,
            1 => FlowKind::Sight,
            2 => FlowKind::Laser,
            3 => FlowKind::Ballistic,
            4 => FlowKind::Funnel,
            5 => FlowKind::Portal,
            255 => FlowKind::Custom(r.u32()?),
            _ => return Err(SerError::InvalidValue("unknown FlowKind tag")),
        })
    }
}

impl Serialize for Traversal {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.kind);
        w.write(&self.requires);
        w.bool(self.reversible);
    }
}

impl Deserialize for Traversal {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Traversal {
            kind: r.read()?,
            requires: r.read()?,
            reversible: r.bool()?,
        })
    }
}

impl Serialize for Volume {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.bounds);
        w.f64(self.clearance);
    }
}

impl Deserialize for Volume {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Volume {
            bounds: r.read()?,
            clearance: r.f64()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::{from_bytes, to_bytes};

    #[test]
    fn volume_accounts_for_clearance_when_fitting() {
        let v = Volume::with_clearance(Aabb::new(Vec3::ZERO, Vec3::splat(2.0)), 1.0);
        assert_eq!(v.clearance(), 1.0);
        // The occupied box is 2³, but 4³ must actually be free.
        assert_eq!(
            v.required_bounds(),
            Aabb::new(Vec3::splat(-1.0), Vec3::splat(3.0))
        );
        assert!(v.fits_within(Aabb::new(Vec3::splat(-2.0), Vec3::splat(4.0))));
        assert!(
            !v.fits_within(Aabb::new(Vec3::ZERO, Vec3::splat(2.0))),
            "a box exactly the size of the bounds leaves no room for clearance"
        );
        // Negative clearance is meaningless and is clamped rather than trusted.
        assert_eq!(
            Volume::with_clearance(Aabb::new(Vec3::ZERO, Vec3::ONE), -5.0).clearance(),
            0.0
        );
    }

    #[test]
    fn constraints_expose_what_the_solver_needs() {
        let dash = ObjectId::derived("token", "blink_dash");
        let c = Constraints::none()
            .and(Constraint::RequiresToken(dash))
            .and(Constraint::MinClearance(2.0))
            .and(Constraint::WithinScopeKind(NodeKind::Space));
        assert_eq!(c.len(), 3);
        assert_eq!(c.required_tokens().collect::<Vec<_>>(), vec![dash]);
        assert!(Constraints::none().is_empty());
    }

    #[test]
    fn one_way_traversals_are_marked() {
        let key = ObjectId::derived("item", "key");
        assert!(Traversal::open(TraversalKind::Walk).reversible);
        assert!(Traversal::gated(TraversalKind::Walk, [key])
            .requires
            .contains(&key));
        // A drop you cannot climb back up — what the un-softlockable pass has to reason about.
        assert!(!Traversal::open(TraversalKind::Jump).one_way().reversible);
    }

    #[test]
    fn unregistered_content_gets_working_defaults_not_an_error() {
        let reg = MechanicRegistry::new();
        let ctx = Context::detached();
        let m = reg.get(ObjectId::derived("actor", "nothing"));
        // The pipeline can call every callback without first asking whether behaviour exists.
        assert!(m.footprint(&ctx).is_none());
        assert!(m.constraints(&ctx).is_empty());
        assert!(m.affords(&ctx).is_empty());
        assert!(m.grants(&ctx).is_none());
        assert!(!m.blocks(&ctx, FlowKind::Walking));
        assert!(m.redirects(&ctx, FlowKind::Laser, Vec3::X).is_none());
        assert_eq!(m.label(), "<default>");
    }

    #[test]
    fn registry_iteration_is_deterministic() {
        let mut reg = MechanicRegistry::new();
        for name in ["zeta", "alpha", "mu"] {
            reg.register(ObjectId::derived("actor", name), Box::new(DefaultMechanic));
        }
        let first: Vec<ObjectId> = reg.ids().collect();
        assert_eq!(first, reg.ids().collect::<Vec<_>>());
        let mut sorted = first.clone();
        sorted.sort();
        assert_eq!(
            first, sorted,
            "ids come out in id order, not insertion order"
        );
        assert_eq!(reg.len(), 3);
    }

    #[test]
    fn value_types_round_trip() {
        let t = Traversal {
            kind: TraversalKind::Custom(42),
            requires: vec![ObjectId::derived("item", "key")],
            reversible: false,
        };
        assert_eq!(from_bytes::<Traversal>(&to_bytes(&t)).unwrap(), t);

        let v = Volume::with_clearance(Aabb::new(Vec3::ZERO, Vec3::new(0.1, 2.5, 3.0)), 0.75);
        assert_eq!(from_bytes::<Volume>(&to_bytes(&v)).unwrap(), v);

        for f in FlowKind::CORE.iter().chain(&[FlowKind::Custom(7)]) {
            assert_eq!(from_bytes::<FlowKind>(&to_bytes(f)).unwrap(), *f);
        }
    }
}
