//! cv-vm — the owned bytecode virtual machine that runs compiled **Schematics** in-process, on both
//! native and WASM. No Rhai; no host FFI beyond the sanctioned Context surface.
//!
//! ⚠ **The input is a compiled node graph, not a text file.** CVScript is a *visual* system: a
//! developer authors Schematics as graphs, and those are compiled to the instruction set this executes.
//! There is no scripting-language source artifact to load.
//!
//! ⚠ And **CVB is a notation, not a file type** — `Begin X … End X`, `Key=Value`, `Pin (…)` — in which
//! `.cvs`, `.cvspine` and `.cvstate` are three *separate* formats written. An artifact named after the
//! notation would be naming the ink rather than the document.
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
