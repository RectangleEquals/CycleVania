//! cv-determinism — the one legal source of randomness and math for the whole engine.
//!
//! Contract (Design/v0.1): *replayable-deterministic* — same seed + same build ⇒ same output; WASM is
//! the canonical cross-machine target. Owned forkable PRNG (`ctx.rng.fork(label)`), owned transcendental
//! math (FMA/fast-math off), ordered iteration, no clock, no ambient RNG.
//!
//! * [`Rng`] — the forkable, label-addressed PRNG (M01).
//! * [`math`] — owned exact + transcendental scalar math, and the float contract (M02).
//! * [`geom`] — `Vec3`/`Quat`/`Transform`/`Mat4`/`Aabb` kernels built on `math` (M02).
//! * [`probe`] — the canonical cross-target determinism blob (M02).

pub mod geom;
pub mod math;
pub mod probe;
mod rng;

pub use geom::{Aabb, Mat4, Quat, Transform, Vec3};
pub use rng::Rng;

/// This crate's version, surfaced for cross-crate linkage smoke tests.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::version().is_empty());
    }
}
