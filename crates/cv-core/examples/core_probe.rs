//! A wasm32 `cdylib` exporting cv-core's determinism probe, so `scripts/wasm-golden.cjs` can compare
//! the data model's serialized output against the same fixture the native test uses.
//!
//! This is what turns "the `Writer` has no `usize` method" into "both targets emit these exact bytes".
//!
//! Build: `cargo build -p cv-core --example wasm_probe --target wasm32-unknown-unknown`

/// Byte length of the probe blob.
#[no_mangle]
pub extern "C" fn probe_len() -> usize {
    cv_core::probe::determinism_probe().len()
}

/// Pointer to the probe blob in linear memory, valid for [`probe_len`] bytes.
///
/// The buffer is intentionally leaked so it stays valid for the module's lifetime — correct for a
/// one-shot probe.
#[no_mangle]
pub extern "C" fn probe_ptr() -> *const u8 {
    let bytes = cv_core::probe::determinism_probe().into_boxed_slice();
    Box::leak(bytes).as_ptr()
}

/// Unused on wasm, but a `cdylib` example still needs a `main` for non-wasm builds.
#[allow(dead_code)]
fn main() {}
