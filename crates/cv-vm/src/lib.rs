//! cv-vm — the owned bytecode virtual machine that runs compiled CVScript (`.cvb`) in-process, on both
//! native and WASM. No Rhai; no host FFI beyond the sanctioned Context surface.
//!
//! **M00: skeleton only.** The interpreter + api-dispatch land in M18 (after the compiler, M15–M17).

/// This crate's version.
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
