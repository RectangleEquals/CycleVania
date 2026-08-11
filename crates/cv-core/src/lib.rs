//! cv-core — the generation engine: the deterministic data model (arena-of-handles graph), the L0–L6
//! pipeline, the scheduling engine (L1), the solver (L2, incl. the no-softlock guarantee), and the
//! Context API. Pure computation, no I/O; the VM (cv-vm) is embedded here for api-dispatch.
//!
//! **M00: skeleton only.** The arena + node graph land in M03/M04; the pipeline in M06–M14.

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
