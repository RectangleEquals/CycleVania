//! **Loading a project from disk** — the descriptor, the content root, and everything under it.
//!
//! ⚠ **Native Rust over the core, never through the TS bindings.** The CLI has to stay usable while the
//! bindings are mid-change, because it is the tool used to *debug* them. A CLI that routed through the
//! bindings could not diagnose the one thing it would most often be asked to.
//!
//! # Folders are configurable; the schematic root is not
//!
//! ⚠ **Only `content/schematics` is a hard rule.** The content root itself and everything else under it
//! are developer-defined, so this reads them from the descriptor rather than assuming a layout — a tool
//! that hardcoded `content/` would silently find nothing in a project that had moved it.

use cv_assets::json::{parse, Json};
use std::fmt;
use std::path::{Path, PathBuf};

/// What a `.cvproj` says.
#[derive(Clone, Debug, PartialEq)]
pub struct Descriptor {
    /// Where the descriptor itself lives.
    pub path: PathBuf,
    /// The core version it was authored against. ⚠ **Part of the fingerprint.**
    pub cyclevania: String,
    /// Units per metre.
    pub world_scale: f64,
    /// The content root, relative to the descriptor.
    pub content_root: String,
}

impl Descriptor {
    /// The absolute content root.
    pub fn content_dir(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or(Path::new("."))
            .join(&self.content_root)
    }

    /// ⚠ **The one fixed subpath.** Everything else is the developer's to arrange.
    pub fn schematic_dir(&self) -> PathBuf {
        self.content_dir().join("schematics")
    }
}

/// Why a project did not load.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    /// No file there.
    NotFound { path: String },
    /// The descriptor did not read as JSON.
    Malformed { path: String, detail: String },
    /// A required member is missing.
    ///
    /// ⚠ **Named individually rather than as *"invalid descriptor"***, because the fix is one line and
    /// the message should say which.
    Missing { path: String, member: String },
    /// The content root the descriptor names does not exist.
    NoContentRoot { path: String },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotFound { path } => write!(f, "{path}: no such project descriptor"),
            LoadError::Malformed { path, detail } => write!(f, "{path}: {detail}"),
            LoadError::Missing { path, member } => write!(f, "{path}: no \"{member}\""),
            LoadError::NoContentRoot { path } => {
                write!(f, "{path}: the content root it names does not exist")
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// Read a `.cvproj`.
pub fn load(path: &Path) -> Result<Descriptor, LoadError> {
    let shown = path.display().to_string();
    let src = std::fs::read_to_string(path).map_err(|_| LoadError::NotFound {
        path: shown.clone(),
    })?;
    let doc = parse(&src).map_err(|e| LoadError::Malformed {
        path: shown.clone(),
        detail: e.to_string(),
    })?;

    let cyclevania = doc
        .get("cyclevania")
        .and_then(Json::as_str)
        .ok_or_else(|| LoadError::Missing {
            path: shown.clone(),
            member: "cyclevania".into(),
        })?
        .to_string();

    // ⚠ **`worldScale` defaults and `cyclevania` does not.** A missing scale has one obvious right
    // answer; a missing version has none, and guessing it would silently fingerprint a project against
    // a core it was never authored for.
    let world_scale = doc.get("worldScale").and_then(Json::as_f64).unwrap_or(1.0);

    let content_root = doc
        .get("paths")
        .and_then(|p| p.get("contentRoot"))
        .and_then(Json::as_str)
        .unwrap_or("content")
        .to_string();

    let descriptor = Descriptor {
        path: path.to_path_buf(),
        cyclevania,
        world_scale,
        content_root,
    };
    if !descriptor.content_dir().is_dir() {
        return Err(LoadError::NoContentRoot { path: shown });
    }
    Ok(descriptor)
}

/// Every file under a directory with one of these extensions, sorted.
///
/// ⚠ **Sorted, because a directory walk's order is the filesystem's.** Two machines scanning the same
/// tree must produce the same list, or every downstream ordering inherits a difference nobody authored.
pub fn files_under(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, extensions, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            walk(&p, extensions, out);
        } else if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            if extensions.iter().any(|e| name.ends_with(e)) {
                out.push(p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cv-cli-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content/schematics")).unwrap();
        dir
    }

    fn write(dir: &Path, rel: &str, body: &str) -> PathBuf {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn a_descriptor_reads_its_version_scale_and_content_root() {
        let dir = scratch("descriptor");
        let proj = write(
            &dir,
            "game.cvproj",
            r#"{"cyclevania":"0.2.0","worldScale":2.0,"paths":{"contentRoot":"content"}}"#,
        );
        let d = load(&proj).unwrap();
        assert_eq!(d.cyclevania, "0.2.0");
        assert_eq!(d.world_scale, 2.0);
        assert_eq!(d.content_dir(), dir.join("content"));
        assert_eq!(d.schematic_dir(), dir.join("content/schematics"));
    }

    #[test]
    fn a_missing_version_is_refused_and_a_missing_scale_defaults() {
        // ⚠ A missing scale has one obvious right answer; a missing version has none, and guessing it
        // would fingerprint a project against a core it was never authored for.
        let dir = scratch("defaults");
        let proj = write(&dir, "game.cvproj", r#"{"cyclevania":"0.2.0"}"#);
        assert_eq!(load(&proj).unwrap().world_scale, 1.0);

        let bad = write(&dir, "no-version.cvproj", r#"{"worldScale":1.0}"#);
        assert!(matches!(
            load(&bad),
            Err(LoadError::Missing { member, .. }) if member == "cyclevania"
        ));
    }

    #[test]
    fn a_relocated_content_root_is_followed_rather_than_assumed() {
        // ⚠ A tool that hardcoded `content/` would silently find nothing in a project that moved it.
        let dir = scratch("relocated");
        std::fs::create_dir_all(dir.join("assets/schematics")).unwrap();
        let proj = write(
            &dir,
            "game.cvproj",
            r#"{"cyclevania":"0.2.0","paths":{"contentRoot":"assets"}}"#,
        );
        let d = load(&proj).unwrap();
        assert_eq!(d.content_dir(), dir.join("assets"));
        assert_eq!(d.schematic_dir(), dir.join("assets/schematics"));
    }

    #[test]
    fn a_content_root_that_does_not_exist_is_an_error_rather_than_an_empty_project() {
        let dir = scratch("noroot");
        let proj = write(
            &dir,
            "game.cvproj",
            r#"{"cyclevania":"0.2.0","paths":{"contentRoot":"nowhere"}}"#,
        );
        assert!(matches!(load(&proj), Err(LoadError::NoContentRoot { .. })));
    }

    #[test]
    fn a_missing_or_malformed_descriptor_is_named() {
        let dir = scratch("bad");
        assert!(matches!(
            load(&dir.join("absent.cvproj")),
            Err(LoadError::NotFound { .. })
        ));
        let broken = write(&dir, "broken.cvproj", "{ not json");
        let err = load(&broken).unwrap_err();
        assert!(matches!(err, LoadError::Malformed { .. }));
        assert!(err.to_string().contains("broken.cvproj"));
    }

    #[test]
    fn a_content_walk_is_sorted_so_two_machines_agree() {
        // ⚠ A directory walk's order is the filesystem's; every downstream ordering would inherit it.
        let dir = scratch("walk");
        write(
            &dir,
            "content/schematics/z.cvs",
            "Begin Schematic\nEnd Schematic\n",
        );
        write(
            &dir,
            "content/schematics/a.cvs",
            "Begin Schematic\nEnd Schematic\n",
        );
        write(
            &dir,
            "content/schematics/sub/m.cvs",
            "Begin Schematic\nEnd Schematic\n",
        );
        write(&dir, "content/schematics/notes.txt", "ignored");

        let found = files_under(&dir.join("content"), &[".cvs"]);
        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["a.cvs", "m.cvs", "z.cvs"]);
    }

    #[test]
    fn a_walk_of_nothing_is_empty_rather_than_a_panic() {
        assert!(files_under(Path::new("/definitely/not/here"), &[".cvs"]).is_empty());
    }
}
