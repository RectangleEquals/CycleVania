//! cv-determinism — the one legal source of randomness and math for the whole engine.
//!
//! Contract (Design/v0.1): *replayable-deterministic* — same seed + same build ⇒ same output; WASM is
//! the canonical cross-machine target. Owned forkable PRNG (`ctx.rng.fork(label)`), owned transcendental
//! math (FMA/fast-math off), ordered iteration, no clock, no ambient RNG.
//!
//! **M01: the owned PRNG (`Rng`) is in.** M02 lands the owned math + cross-target golden vectors.

mod rng;
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
