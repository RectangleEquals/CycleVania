//! **Content across the seam** — list, read, write.
//!
//! ⚠ **The editor is the reason this exists, and it is not the editor's.** Opening a project, listing
//! what is in it, reading a file and writing it back is what an authoring tool does — and every one of
//! those is a question about a *project*, so a host may ask it too. The editor is only the first caller.
//!
//! # Writing goes through the canonical writer, always
//!
//! ⚠ **A second formatter is a second dialect.** If the editor serialised `.cvs` itself, two writers
//! would exist and would drift — and the drift would surface as a diff in someone's version control
//! rather than as a failing test. So a write here is **parse, then re-emit**: text in, `Block` out,
//! canonical text back.
//!
//! ▶ **Which makes byte-identity a property rather than a hope.** Writing a file that was already
//! canonical changes nothing, and writing one that was not normalises it — both are the same code path,
//! so neither is a special case anybody has to remember.

use cv_assets::project::{files_under, Descriptor};
use std::fmt;
use std::path::{Path, PathBuf};

/// Every extension the content root may hold.
///
/// ⚠ **The content set from the design, and only it.** `.cvproj` is *not* here: the descriptor sits
/// outside the content root, because it is the file that says where the content root is.
pub const CONTENT: &[&str] = &["cvs", "cvspine", "cvstate", "cvcurve", "cvunlock", "cvtags"];

/// Why a content operation did not happen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentError {
    /// No such file under the content root.
    NotFound {
        /// The path as asked for.
        rel: String,
    },
    /// A path that leaves the content root.
    ///
    /// ⚠ **Refused rather than resolved.** `../../etc/passwd` is a path an editor can be talked into
    /// sending, and a tool that serves any file it can open is a tool that serves any file it can open.
    Escapes {
        /// The path as asked for.
        rel: String,
    },
    /// The text did not parse as CVB.
    Malformed {
        /// The path as asked for.
        rel: String,
        /// What the parser said.
        detail: String,
    },
    /// The write could not be completed.
    Io {
        /// The path as asked for.
        rel: String,
        /// What the filesystem said.
        detail: String,
    },
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentError::NotFound { rel } => write!(f, "no content file `{rel}`"),
            ContentError::Escapes { rel } => {
                write!(f, "`{rel}` leaves the content root, which is not served")
            }
            ContentError::Malformed { rel, detail } => {
                write!(f, "`{rel}` did not parse: {detail}")
            }
            ContentError::Io { rel, detail } => write!(f, "`{rel}`: {detail}"),
        }
    }
}

impl std::error::Error for ContentError {}

/// Resolve a caller-supplied relative path against the content root.
///
/// ⚠ **The escape check is on the *resolved* path, not the text.** Rejecting strings containing `..` is
/// the check that looks right and misses `a/b/../../..`; comparing the normalised result against the
/// root is the one that holds. Symlinks are out of scope — the content root is the developer's own tree.
fn under(root: &Path, rel: &str) -> Result<PathBuf, ContentError> {
    let mut out = root.to_path_buf();
    for part in rel.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                if !out.pop() || !out.starts_with(root) {
                    return Err(ContentError::Escapes { rel: rel.into() });
                }
            }
            p => out.push(p),
        }
    }
    if out.starts_with(root) {
        Ok(out)
    } else {
        Err(ContentError::Escapes { rel: rel.into() })
    }
}

/// Every content file in the project, as paths relative to the content root.
///
/// ⚠ **Relative and sorted.** Absolute paths would leak the developer's directory layout into anything
/// that displays or stores the list, and an unsorted walk is the filesystem's order rather than one
/// anybody chose — the same reason [`files_under`] sorts.
pub fn list(descriptor: &Descriptor) -> Vec<String> {
    let root = descriptor.content_dir();
    files_under(&root, CONTENT)
        .into_iter()
        .filter_map(|p| {
            p.strip_prefix(&root)
                .ok()
                .map(|r| r.to_string_lossy().replace('\\', "/"))
        })
        .collect()
}

/// Read one content file.
pub fn read(descriptor: &Descriptor, rel: &str) -> Result<String, ContentError> {
    let path = under(&descriptor.content_dir(), rel)?;
    std::fs::read_to_string(&path).map_err(|_| ContentError::NotFound { rel: rel.into() })
}

/// Write one content file, **canonically**.
///
/// ⚠ **Parse first, and fail on text that does not.** Writing unparseable text would put a file in the
/// content root that the next `list` offers and the next `read` returns and nothing can load — a fault
/// stored now and discovered somewhere else.
pub fn write(descriptor: &Descriptor, rel: &str, src: &str) -> Result<String, ContentError> {
    let path = under(&descriptor.content_dir(), rel)?;
    let block = cv_cvb::parse(src).map_err(|e| ContentError::Malformed {
        rel: rel.into(),
        detail: e.to_string(),
    })?;
    let canonical = cv_cvb::write(&block);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| ContentError::Io {
            rel: rel.into(),
            detail: e.to_string(),
        })?;
    }
    std::fs::write(&path, &canonical).map_err(|e| ContentError::Io {
        rel: rel.into(),
        detail: e.to_string(),
    })?;
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        std::env::temp_dir().join("cv-content-test")
    }

    fn descriptor() -> Descriptor {
        let base = root();
        let _ = std::fs::create_dir_all(base.join("content"));
        Descriptor {
            path: base.join("game.cvproj"),
            cyclevania: "0.1.0".into(),
            world_scale: 1.0,
            content_root: "content".into(),
        }
    }

    #[test]
    fn a_path_that_leaves_the_content_root_is_refused() {
        // ⚠ The check is on the resolved path, so a walk that climbs out and back is still caught.
        let d = descriptor();
        for bad in ["../secrets", "a/../../secrets", "../../etc/passwd"] {
            assert!(
                matches!(read(&d, bad), Err(ContentError::Escapes { .. })),
                "{bad} was served"
            );
        }
    }

    #[test]
    fn a_path_that_climbs_and_returns_stays_inside() {
        // ⚠ Not every `..` is an escape — refusing them all would refuse correct paths.
        let d = descriptor();
        let root = d.content_dir();
        assert_eq!(under(&root, "a/../b.cvs").unwrap(), root.join("b.cvs"));
    }

    #[test]
    fn writing_normalises_and_writing_again_changes_nothing() {
        // ⚠ Byte-identity is the property M16's round-trip check depends on.
        let d = descriptor();
        // ⚠ Deliberately out of canonical order — and deliberately not behind a fallible binding.
        // An early return here would let this test pass by doing nothing, which is the exact failure
        // mode it exists to rule out.
        let messy = "Begin X Id=x Version=1
   B=2
   A=1
End X
";
        let once = write(&d, "round.cvs", messy).expect("a valid block writes");
        assert_ne!(once, messy, "the sample must actually need normalising");
        let twice = write(&d, "round.cvs", &once).expect("canonical text re-writes");
        assert_eq!(once, twice, "writing canonical text must be a no-op");
        assert_eq!(read(&d, "round.cvs").unwrap(), once);
    }

    #[test]
    fn text_that_does_not_parse_is_refused_rather_than_stored() {
        let d = descriptor();
        let err = write(&d, "bad.cvs", "Begin Unclosed\n").unwrap_err();
        assert!(matches!(err, ContentError::Malformed { .. }), "{err}");
        assert!(
            matches!(read(&d, "bad.cvs"), Err(ContentError::NotFound { .. })),
            "a refused write must not leave a file behind"
        );
    }

    #[test]
    fn listing_is_relative_and_sorted() {
        let d = descriptor();
        let content = d.content_dir();
        let _ = std::fs::create_dir_all(content.join("nested"));
        let _ = std::fs::write(content.join("b.cvs"), "");
        let _ = std::fs::write(content.join("a.cvs"), "");
        let _ = std::fs::write(content.join("nested/c.cvspine"), "");
        let _ = std::fs::write(content.join("ignored.txt"), "");
        let files = list(&d);
        assert!(files
            .iter()
            .all(|f| !f.starts_with('/') && !f.contains(':')));
        assert!(!files.iter().any(|f| f.ends_with(".txt")), "{files:?}");
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted, "the list must not depend on the filesystem");
    }
}
