//! cv-core — the generation engine: the deterministic data model (arena-of-handles graph), the L0–L6
//! pipeline, the scheduling engine (L1), the solver (L2, incl. the no-softlock guarantee), and the
//! Context API. Pure computation, no I/O; the VM (cv-vm) is embedded here for api-dispatch.
//!
//! **M03–M04: the data model's foundation.**
//!
//! * [`arena`] — the generational [`Arena`] and typed [`Handle`]s every object lives in.
//! * [`object`] — [`ObjectId`] identity, [`ObjectHeader`], and the [`Object`] trait.
//! * [`node`] — the `World → Reach → Area → Space → Spatial` scope graph and its
//!   projected→reserved→realized lifecycle.
//! * [`settings`] — project settings, and `world_scale`: the units every spatial quantity is in.
//! * [`content`] — the L0 registry of everything a world may be built from.
//! * [`fingerprint`] — recipe identity (deliberately excluding the seed) and reproduction bundles.
//! * [`descriptor`] — the host-facing `WorldDescriptor`: what goes where and why, never geometry.
//! * [`events`] — the one-way, batched notification bridge to the host.
//! * [`context`] — the per-call lens handed into every authored callback.
//! * [`unlock`] — the progression vocabulary: `Unlock` rows and their `supersedes` ordering.
//! * [`schedule`] — L0 content resolution and the L1 plan, including `AdaptiveRange`.
//! * [`mission`] — the L2 mission graph, the `Rule` grammar, and sphere reachability.
//! * [`solver`] — assumed-fill placement and cycle generation.
//! * [`softlock`] — the un-softlockable guarantee: no accessible state strands the goal.
//! * [`spine`] — opt-in macro-structure: guaranteed slots, free-form segments.
//! * [`geometry`] — coarse colliders and the spatial primitives content reasons through.
//! * [`serialize`] — the deterministic binary spine for reproduction bundles and round-trips.
//! * [`probe`] — the cross-target determinism blob for the data model.
//!
//! ⚠ **Milestone numbers are deliberately absent.** They used to appear throughout and cited the
//! **v0.1** plan, which v0.2b reuses for unrelated work — `` meant the mission graph there and
//! means dials here. Stripped at M04a; the plan is the place that tracks the plan.

// No `unsafe` anywhere in the core. The arena's stale-handle guarantee is an explicit generation
// check rather than a pointer trick, so it is enforced by the compiler and by tests — which is a
// stronger statement than "miri found no UB", and it holds on the pinned stable toolchain where miri
// is not available.
#![forbid(unsafe_code)]

pub mod arena;
pub mod collision;
pub mod component;
pub mod content;
pub mod context;
pub mod descriptor;
pub mod events;
pub mod fingerprint;
pub mod floor;
pub mod geometry;
pub mod intra;
pub mod judge;
pub mod lifecycle;
pub mod mission;
pub mod node;
pub mod object;
pub mod placement;
pub mod probe;
pub mod query;
pub mod schedule;
pub mod serialize;
pub mod settings;
pub mod shape;
pub mod softlock;
pub mod solver;
pub mod spine;
pub mod surface;
pub mod tag;
pub mod trivalent;
pub mod unlock;

pub use arena::{Arena, Handle};
pub use collision::{CollisionBody, CollisionData, CollisionLayer};
pub use component::{Attached, CollisionMode, Component, Components, Direction};
pub use content::{ContentEntry, ContentKind, ContentRegistry, RegistryError};
pub use context::Context;
pub use descriptor::{
    DescriptorBuilder, InstanceRecord, MeshRecord, Placement, PlacementReason, Rationale,
    ScopeRecord, ScopeRef, Socket, SpineSlotTag, WorldDescriptor,
};
pub use events::{EventLog, GenEvent, Verbosity};
pub use fingerprint::{Fingerprint, FingerprintBuilder, ReproductionBundle, ReproductionError};
pub use floor::{ConvexHull, FloorSurface, ScopeBounds};
pub use geometry::{CoarseGeometry, Collider, ColliderId, Face, Hit, Sweep};
pub use intra::{EdgeSource, FloorEdge, IntraSpace};
pub use judge::{Budget, Obligation, Path, PathStep, Route, Verdict};
pub use lifecycle::{Event, Quantity, Replenish};
pub use mission::{
    Accessibility, EdgeSpan, Location, LocationId, MissionEdge, MissionGraph, Rule, Sphere,
};
pub use node::{InstanceScope, Node, NodeError, NodeGraph, NodeKind, NodeResult, NodeState};
pub use object::{IdAllocator, Object, ObjectHeader, ObjectId};
pub use placement::{
    Constraint, DirectionCone, Interaction, ItemClass, Preference, Role, RoleEvidence, ScheduleRule,
};
pub use query::{Consider, Detail, Query, Trace};
pub use schedule::{
    AdaptiveRange, Candidate, ContentPool, CountRule, Curve, PlannedSlot, PoolEntry, Progression,
    Schedule, ScheduleBook, SchedulePlan, Scheduler, ScopeFilter, SeedPolicy, SlotRule, Span,
    TargetOutcome, TargetReasoning, WorldLimit,
};
pub use serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
pub use shape::{Shape, ShapeFamily};
pub use softlock::{
    AnalysisLimit, Repair, Softlock, SoftlockAnalysis, SoftlockAnalyzer, SoftlockKind,
};
pub use solver::{PlacementTrace, Solution, SolveError, Solver};
pub use spine::{
    Coverage, GrantSpec, Relaxation, SlotAssignment, SlotContents, SlotRole, SlotShape, SpineError,
    SpineInstance, SpineInstantiator, SpineSegment, SpineSlot, SpineTemplate, SpineValidation,
    SpineWarning, Strictness, UnlockRef,
};
pub use surface::{Approach, AttemptKind, Harm, Occupant, Support, Surface};
pub use tag::{Tag, TagQuery};
pub use trivalent::{within, Fidelity, Tolerances, Trivalent};
pub use unlock::{GrantMap, TableError, Unlock, UnlockTable};

/// This crate's version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Cross-crate linkage smoke: prove cv-core links cv-determinism and cv-vm.
pub fn deps() -> (&'static str, &'static str) {
    (cv_determinism::version(), cv_vm::version())
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::version().is_empty());
    }

    #[test]
    fn links_lower_crates() {
        let (det, vm) = super::deps();
        assert!(!det.is_empty() && !vm.is_empty());
    }
}
