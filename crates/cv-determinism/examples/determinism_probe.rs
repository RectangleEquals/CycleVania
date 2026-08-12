//! A wasm32 `cdylib` exporting the determinism probe blob, so the Node harness
//! (`scripts/wasm-golden.cjs`) can read it out of linear memory and compare it against the *same*
//! golden fixture the native test uses. This is how the cross-target guarantee is actually verified.
//!
//! Build: `cargo build -p cv-determinism --example wasm_probe --target wasm32-unknown-unknown`
//!
//! The two exports each recompute the blob; that is fine precisely *because* it is deterministic.
//! `probe_ptr` leaks its buffer, which is correct for a one-shot probe module.

/// Byte length of the probe blob.
///
/// # Safety
/// Exported for the Node harness; takes no arguments and touches no shared state.
#[no_mangle]
pub extern "C" fn probe_len() -> usize {
    cv_determinism::probe::determinism_probe().len()
}

/// Pointer to the probe blob inside linear memory, valid for [`probe_len`] bytes.
///
/// # Safety
/// The buffer is intentionally leaked so the pointer stays valid for the module's lifetime.
#[no_mangle]
pub extern "C" fn probe_ptr() -> *const u8 {
    let bytes = cv_determinism::probe::determinism_probe().into_boxed_slice();
    Box::leak(bytes).as_ptr()
}

/// Unused on wasm, but a `cdylib` example still needs a `main` for non-wasm builds.
#[allow(dead_code)]
fn main() {}
