//! cv-bindings — the host-facing surface, built from one source into a native Node addon (napi-rs v3)
//! and a WASM module (wasm-bindgen), feature-gated so neither target drags in the other's toolchain.
//! The two `version` exports are cfg-gated to mutually exclusive targets, so they never coexist.
//!
//! **M00: a `version()` smoke export on each target.** The real surface — `load`, `generate`, the
//! runtime dials/assets interface, and the notification bridge — lands in M21.

/// Target-agnostic implementation shared by both bindings.
fn core_version() -> String {
    format!(
        "cyclevania {} (core {}, determinism {})",
        env!("CARGO_PKG_VERSION"),
        cv_core::version(),
        cv_determinism::version(),
    )
}

// --- Native Node addon (napi-rs v3) ---
#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
use napi_derive::napi;

#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
#[napi]
pub fn version() -> String {
    core_version()
}

// --- WASM module (wasm-bindgen) ---
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen]
pub fn version() -> String {
    core_version()
}
