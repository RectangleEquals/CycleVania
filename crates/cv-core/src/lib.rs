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
//! * [`mechanic`] — the api-shaped callback seam that breaks the core↔CVScript cycle (M07).
//! * [`context`] — the per-call lens handed into every mechanic callback.
//! * [`fixtures`] — hand-written mechanics standing in for CVScript until the VM lands (M18).
//! * [`schedule`] — L0 content resolution and the L1 plan, including `AdaptiveRange` (M08).
//! * [`mission`] — the L2 mission graph, the `Rule` grammar, and sphere reachability (M09).
//! * [`solver`] — assumed-fill placement, cycle generation, and the linearity dials (M09).
//! * [`softlock`] — the un-softlockable guarantee: no accessible state strands the goal (M10).
//! * [`spine`] — opt-in macro-structure: guaranteed slots, free-form segments (M10a).
//! * [`geometry`] — coarse colliders and the spatial primitives mechanics reason through (M11).
//! * [`serialize`] — the deterministic binary spine for reproduction bundles and round-trips.
//! * [`probe`] — the cross-target determinism blob for the data model.
//!
//! The pipeline (M06–M14) builds on these.

// No `unsafe` anywhere in the core. The arena's stale-handle guarantee is an explicit generation
// check rather than a pointer trick, so it is enforced by the compiler and by tests — which is a
// stronger statement than "miri found no UB", and it holds on the pinned stable toolchain where miri
// is not available.
#![forbid(unsafe_code)]

pub mod arena;
pub mod content;
pub mod context;
pub mod descriptor;
pub mod events;
pub mod fingerprint;
pub mod fixtures;
pub mod geometry;
pub mod mechanic;
pub mod mission;
pub mod node;
pub mod object;
pub mod probe;
pub mod schedule;
pub mod serialize;
pub mod settings;
pub mod softlock;
pub mod solver;
pub mod spine;
pub mod unlock;

pub use arena::{Arena, Handle};
pub use content::{ContentEntry, ContentKind, ContentRegistry, RegistryError};
pub use context::Context;
pub use descriptor::{
    DescriptorBuilder, InstanceRecord, MeshRecord, Placement, PlacementReason, Rationale,
    ScopeRecord, ScopeRef, Socket, SpineSlotTag, WorldDescriptor,
};
pub use events::{EventLog, GenEvent, Verbosity};
pub use fingerprint::{Fingerprint, FingerprintBuilder, ReproductionBundle, ReproductionError};
pub use geometry::{CoarseGeometry, Collider, ColliderId, Face, Hit, Sweep};
pub use mechanic::{
    Constraint, Constraints, DefaultMechanic, FlowKind, Mechanic, MechanicRegistry, Request,
    Traversal, TraversalKind, Volume,
};
pub use mission::{Accessibility, Location, LocationId, MissionEdge, MissionGraph, Rule, Sphere};
pub use node::{Node, NodeError, NodeGraph, NodeKind, NodeResult, NodeState};
pub use object::{IdAllocator, Object, ObjectHeader, ObjectId};
pub use schedule::{
    AdaptiveRange, Candidate, ContentPool, CountRule, Curve, PlannedSlot, PoolEntry, Progression,
    Schedule, ScheduleBook, SchedulePlan, Scheduler, ScopeFilter, SeedPolicy, SlotRule, Span,
    TargetOutcome, TargetReasoning, WorldLimit,
};
pub use serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
pub use softlock::{
    AnalysisLimit, Repair, Softlock, SoftlockAnalysis, SoftlockAnalyzer, SoftlockKind,
};
pub use solver::{
    Linearity, LinearityOverride, LinearityResolver, PlacementTrace, Solution, SolveError, Solver,
};
pub use spine::{
    Coverage, GrantSpec, Relaxation, SlotAssignment, SlotContents, SlotRole, SlotShape, SpineError,
    SpineInstance, SpineInstantiator, SpineSegment, SpineSlot, SpineTemplate, SpineValidation,
    SpineWarning, Strictness, UnlockRef,
};
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
