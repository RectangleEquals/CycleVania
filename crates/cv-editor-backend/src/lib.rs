//! cv-editor-backend — the WebSocket service that wraps cv-core + cv-script for the browser editor
//! (the Tauri build links the core in-process and skips this). Native only.
//!
//! **M00: skeleton only.** The service, transport parity, and LAN remote/auth land in M22.

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
