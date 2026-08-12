//! cv-core — the generation engine: the deterministic data model (arena-of-handles graph), the L0–L6
//! pipeline, the scheduling engine (L1), the solver (L2, incl. the no-softlock guarantee), and the
//! Context API. Pure computation, no I/O; the VM (cv-vm) is embedded here for api-dispatch.
//!
//! **M03: the data model's foundation.**
//!
//! * [`arena`] — the generational [`Arena`] and typed [`Handle`]s every object lives in.
//! * [`object`] — [`ObjectId`] identity, [`ObjectHeader`], and the [`Object`] trait.
//! * [`serialize`] — the deterministic binary spine for reproduction bundles and round-trips.
//! * [`probe`] — the cross-target determinism blob for the data model.
//!
//! The pipeline (M06–M14) and the node graph (M04) build on these.

// No `unsafe` anywhere in the core. The arena's stale-handle guarantee is an explicit generation
// check rather than a pointer trick, so it is enforced by the compiler and by tests — which is a
// stronger statement than "miri found no UB", and it holds on the pinned stable toolchain where miri
// is not available.
#![forbid(unsafe_code)]

pub mod arena;
pub mod object;
pub mod probe;
pub mod serialize;

pub use arena::{Arena, Handle};
pub use object::{IdAllocator, Object, ObjectHeader, ObjectId};
pub use serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};

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
