//! Shared golden-vector helper for cv-determinism integration tests.

use std::path::PathBuf;

/// Resolve a fixture under the workspace-root `golden/vectors/` dir.
pub fn golden_path(name: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/cv-determinism; the golden dir is two levels up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/vectors")
        .join(name)
}

/// Byte-compare `actual` against the committed golden fixture `name`.
///
/// Set `CV_BLESS=1` to (re)write the fixture from `actual` instead of comparing — the deliberate act
/// of regenerating a golden vector. Without it, a missing fixture or any drift fails the test.
pub fn assert_golden_bytes(name: &str, actual: &[u8]) {
    let path = golden_path(name);
    if std::env::var_os("CV_BLESS").is_some() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read(&path).unwrap_or_else(|e| {
        panic!("missing golden fixture {} ({e}); run with CV_BLESS=1 to create", path.display())
    });
    assert_eq!(expected, actual, "golden mismatch for {name}");
}
