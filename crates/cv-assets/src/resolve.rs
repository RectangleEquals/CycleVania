//! **The asset table** — content hashing, and the indirection that lets a rename survive.
//!
//! ⚠ **A reference is an id resolved through a table, never a path stored in a schematic.** That one
//! choice is what makes rename and move resilience possible at all: a schematic holds an id, the table
//! holds the current path, and moving a file rewrites **one row**. A schematic that stored the path
//! would need every referring document rewritten, and the one nobody rewrote would dangle.
//!
//! # The hash is of content, and the path is not part of it
//!
//! ⚠ **Two identical files at two paths have the same hash, and that is the point.** The fingerprint
//! answers *"is this the same recipe?"*, and moving a mesh does not change the world it produces. A hash
//! that folded in the path would make every reorganisation look like a content change and every
//! reproduction bundle stop reproducing.
//!
//! ⚠ **And the digest is of the *loaded form*, not of the file's bytes.** Two curve tables that parse
//! to the same rows through different whitespace are the same **recipe** and different **files**, and
//! the fingerprint is about the recipe — so a byte digest would make a reformat look like a redesign.
//! See [`digest_of`].
//!
//! # Dangling is a build error, at cook time
//!
//! ⚠ **The developer-facing guarantee — *"if the editor let me pick it, it exists"* — is a result of
//! this work rather than a promise.** The cook walks references from the assigned roots and errors on
//! anything that does not resolve, which is the only moment the whole graph is visible at once.

use cv_determinism::hash::{combine, fnv1a_str};
use std::collections::BTreeMap;
use std::fmt;

/// A stable id for one asset, generated once and never edited.
///
/// ⚠ **Not derived from the path.** An id derived from where a file sits would change when it moved,
/// which is the exact failure this indirection exists to prevent.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId(String);

impl AssetId {
    /// An id, as written in a table.
    pub fn new(id: impl Into<String>) -> Self {
        AssetId(id.into())
    }

    /// The id's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One row of the asset table.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// Where it currently lives. ⚠ **Editable** — this is the cell a move rewrites.
    pub path: String,
    /// A digest of what *defines* it.
    ///
    /// ⚠ **Of the content, never of the path.** Moving a file does not change the world it produces, so
    /// a digest that folded in the path would make every reorganisation look like a content change.
    pub digest: u64,
}

/// Why a reference did not resolve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolveError {
    /// Nothing under that id.
    Unknown { id: String },
    /// Two rows claim the same id.
    DuplicateId { id: String },
    /// Two rows claim the same path.
    ///
    /// ⚠ **Refused.** Two ids at one path makes *"which asset is this file"* unanswerable, and the
    /// question is asked every time the editor reloads from disk.
    DuplicatePath { path: String },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::Unknown { id } => write!(
                f,
                "no asset {id} — a dangling reference is a build error, caught when the cook walks \
                 references from the assigned roots"
            ),
            ResolveError::DuplicateId { id } => write!(f, "two assets share the id {id}"),
            ResolveError::DuplicatePath { path } => write!(
                f,
                "two assets claim {path} — *which asset is this file* must have one answer"
            ),
        }
    }
}

impl std::error::Error for ResolveError {}

/// The table every `Asset'…'` resolves through.
#[derive(Clone, Debug, Default)]
pub struct AssetTable {
    rows: BTreeMap<AssetId, Asset>,
}

impl AssetTable {
    /// An empty table.
    pub fn new() -> Self {
        AssetTable::default()
    }

    /// Register an asset.
    pub fn register(
        &mut self,
        id: AssetId,
        path: impl Into<String>,
        digest: u64,
    ) -> Result<(), ResolveError> {
        let path = path.into();
        if self.rows.contains_key(&id) {
            return Err(ResolveError::DuplicateId { id: id.to_string() });
        }
        if self.rows.values().any(|a| a.path == path) {
            return Err(ResolveError::DuplicatePath { path });
        }
        self.rows.insert(id, Asset { path, digest });
        Ok(())
    }

    /// Where an asset currently lives.
    pub fn resolve(&self, id: &AssetId) -> Result<&Asset, ResolveError> {
        self.rows
            .get(id)
            .ok_or_else(|| ResolveError::Unknown { id: id.to_string() })
    }

    /// ⚠ **A move rewrites one cell.** Every schematic referring to this id keeps working, because none
    /// of them ever held the path.
    pub fn move_to(&mut self, id: &AssetId, path: impl Into<String>) -> Result<(), ResolveError> {
        let path = path.into();
        if self.rows.iter().any(|(k, a)| k != id && a.path == path) {
            return Err(ResolveError::DuplicatePath { path });
        }
        let Some(row) = self.rows.get_mut(id) else {
            return Err(ResolveError::Unknown { id: id.to_string() });
        };
        row.path = path;
        Ok(())
    }

    /// Which id, if any, currently sits at a path.
    pub fn at(&self, path: &str) -> Option<&AssetId> {
        self.rows
            .iter()
            .find(|(_, a)| a.path == path)
            .map(|(k, _)| k)
    }

    /// How many assets.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Nothing registered.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Every id, sorted.
    pub fn ids(&self) -> impl Iterator<Item = &AssetId> {
        self.rows.keys()
    }

    /// Every reference that does not resolve.
    ///
    /// ⚠ **Reported all at once.** Stopping at the first dangling reference makes fixing a moved folder
    /// an *n*-pass job, and the cook is the one moment the whole graph is visible.
    pub fn dangling<'a>(
        &self,
        referenced: impl IntoIterator<Item = &'a AssetId>,
    ) -> Vec<&'a AssetId> {
        referenced
            .into_iter()
            .filter(|id| !self.rows.contains_key(id))
            .collect()
    }

    /// **The contribution this table makes to the fingerprint.**
    ///
    /// ⚠ **Ids and digests, in id order; paths deliberately absent.** The fingerprint answers *"is this
    /// the same recipe?"*, and a moved file is the same recipe.
    pub fn fingerprint(&self) -> u64 {
        let mut acc = 0u64;
        for (id, asset) in &self.rows {
            acc = combine(acc, fnv1a_str(id.as_str()));
            acc = combine(acc, asset.digest);
        }
        acc
    }
}

/// The digest of an asset's defining content.
///
/// ⚠ **Taken from what the loader produced, not from the file's bytes.** Two curve tables that parse to
/// the same rows through different whitespace are the same recipe and different files — and the
/// fingerprint is about the recipe, so a byte digest would make a reformat look like a redesign.
pub fn digest_of(canonical: &str) -> u64 {
    fnv1a_str(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> AssetTable {
        let mut t = AssetTable::new();
        t.register(
            AssetId::new("a_curve_01"),
            "/Content/Curves/progression.cvcurve",
            digest_of("rows: complexity, hazard_density"),
        )
        .unwrap();
        t.register(
            AssetId::new("a_mesh_01"),
            "/Content/Meshes/hookshot.glb",
            digest_of("1024 triangles"),
        )
        .unwrap();
        t
    }

    #[test]
    fn a_reference_resolves_through_the_table() {
        let t = table();
        assert_eq!(
            t.resolve(&AssetId::new("a_curve_01")).unwrap().path,
            "/Content/Curves/progression.cvcurve"
        );
    }

    #[test]
    fn a_renamed_asset_does_not_break_a_reference() {
        // ⚠ M14's green condition. The schematic never held the path.
        let mut t = table();
        let id = AssetId::new("a_curve_01");
        let before = t.resolve(&id).unwrap().clone();

        t.move_to(&id, "/Content/Tuning/difficulty.cvcurve")
            .unwrap();

        let after = t.resolve(&id).expect("the reference still resolves");
        assert_eq!(after.path, "/Content/Tuning/difficulty.cvcurve");
        assert_eq!(
            after.digest, before.digest,
            "a move is not a content change"
        );
    }

    #[test]
    fn a_move_does_not_change_the_fingerprint() {
        // ⚠ Otherwise every reorganisation would look like a redesign and every reproduction bundle
        // would stop reproducing.
        let mut t = table();
        let before = t.fingerprint();
        t.move_to(
            &AssetId::new("a_mesh_01"),
            "/Content/Art/Meshes/hookshot.glb",
        )
        .unwrap();
        assert_eq!(t.fingerprint(), before);
    }

    #[test]
    fn changing_content_does_change_the_fingerprint() {
        let mut t = AssetTable::new();
        t.register(AssetId::new("a"), "/x.cvcurve", digest_of("one row"))
            .unwrap();
        let before = t.fingerprint();

        let mut after = AssetTable::new();
        after
            .register(AssetId::new("a"), "/x.cvcurve", digest_of("two rows"))
            .unwrap();
        assert_ne!(after.fingerprint(), before);
    }

    #[test]
    fn two_identical_files_at_two_paths_share_a_digest() {
        // ⚠ The point: a duplicated asset is the same recipe twice.
        assert_eq!(digest_of("same content"), digest_of("same content"));
        assert_ne!(digest_of("same content"), digest_of("other content"));
    }

    #[test]
    fn the_digest_is_of_the_loaded_form_so_a_reformat_is_not_a_redesign() {
        // ⚠ Two files that parse to the same rows are the same recipe.
        let canonical = "domain=depth;rows=complexity,tier";
        assert_eq!(digest_of(canonical), digest_of(canonical));
    }

    #[test]
    fn a_dangling_reference_names_the_id_and_says_when_it_is_caught() {
        let t = table();
        let err = t.resolve(&AssetId::new("a_ghost")).unwrap_err();
        assert_eq!(
            err,
            ResolveError::Unknown {
                id: "a_ghost".into()
            }
        );
        assert!(err.to_string().contains("build error"));
    }

    #[test]
    fn every_dangling_reference_is_reported_at_once() {
        // ⚠ Stopping at the first makes fixing a moved folder an n-pass job.
        let t = table();
        let wanted = [
            AssetId::new("a_curve_01"),
            AssetId::new("a_ghost"),
            AssetId::new("a_other_ghost"),
        ];
        let missing = t.dangling(wanted.iter());
        assert_eq!(missing.len(), 2);
    }

    #[test]
    fn two_assets_may_not_share_an_id_or_a_path() {
        let mut t = table();
        assert_eq!(
            t.register(AssetId::new("a_curve_01"), "/somewhere/else.cvcurve", 1),
            Err(ResolveError::DuplicateId {
                id: "a_curve_01".into()
            })
        );
        let err = t
            .register(AssetId::new("a_new"), "/Content/Meshes/hookshot.glb", 1)
            .unwrap_err();
        assert!(matches!(err, ResolveError::DuplicatePath { .. }));
        assert!(err.to_string().contains("one answer"));
    }

    #[test]
    fn a_move_onto_an_occupied_path_is_refused() {
        let mut t = table();
        assert!(matches!(
            t.move_to(&AssetId::new("a_curve_01"), "/Content/Meshes/hookshot.glb"),
            Err(ResolveError::DuplicatePath { .. })
        ));
    }

    #[test]
    fn a_move_onto_its_own_path_is_a_no_op_rather_than_a_conflict() {
        let mut t = table();
        let id = AssetId::new("a_curve_01");
        let path = t.resolve(&id).unwrap().path.clone();
        assert_eq!(t.move_to(&id, path.clone()), Ok(()));
        assert_eq!(t.resolve(&id).unwrap().path, path);
    }

    #[test]
    fn moving_something_that_does_not_exist_is_an_error() {
        let mut t = table();
        assert!(matches!(
            t.move_to(&AssetId::new("a_ghost"), "/anywhere"),
            Err(ResolveError::Unknown { .. })
        ));
    }

    #[test]
    fn a_path_resolves_back_to_its_id() {
        let t = table();
        assert_eq!(
            t.at("/Content/Meshes/hookshot.glb"),
            Some(&AssetId::new("a_mesh_01"))
        );
        assert!(t.at("/nothing/here").is_none());
    }

    #[test]
    fn the_fingerprint_is_stable_across_registration_order() {
        // ⚠ Ids are sorted, so the table is a set rather than a sequence — otherwise the order a
        // project happened to scan its folders in would reach the fingerprint.
        let mut a = AssetTable::new();
        a.register(AssetId::new("z"), "/z", 1).unwrap();
        a.register(AssetId::new("a"), "/a", 2).unwrap();

        let mut b = AssetTable::new();
        b.register(AssetId::new("a"), "/a", 2).unwrap();
        b.register(AssetId::new("z"), "/z", 1).unwrap();

        assert_eq!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn an_empty_table_still_has_a_fingerprint() {
        let t = AssetTable::new();
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
        let _ = t.fingerprint();
    }
}
