//! cv-script — the CVScript compiler: lexer, parser, semantic analyzer, the determinism +
//! api-signature checkers, and the bytecode compiler that emits `.cvb`. Kept separate from cv-vm so the
//! editor can analyze scripts (autocomplete/errors/headers) without pulling in the pipeline.
//!
//! **M00: skeleton only.** The lexer/parser land in M15; analysis/checkers M16; bytecode M17.

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
