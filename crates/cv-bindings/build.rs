//! Build script. Only the native napi addon needs Node delay-load linker setup; skip it for the WASM
//! target and for non-addon builds so `cargo build --target wasm32 --features wasm` stays clean.
fn main() {
    let napi_addon = std::env::var_os("CARGO_FEATURE_NAPI_ADDON").is_some();
    let is_wasm = std::env::var("CARGO_CFG_TARGET_ARCH")
        .map(|arch| arch == "wasm32")
        .unwrap_or(false);
    if napi_addon && !is_wasm {
        napi_build::setup();
    }
}
