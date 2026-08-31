//! The unlock table — the project's progression vocabulary (L0).
//!
//! # What an unlock is, and what it deliberately is not
//!
//! An **unlock** is one atom of the progression lattice: something an occupant holds or knows. It is a
//! **row of a table**, not a class and not a file of its own — `id`, `name`, `doc`, `supersedes`, and
//! **no behaviour whatever**.
//!
//! That last part is the whole design. Every mechanical consequence — what a rope reaches, what a bomb
//! breaks — belongs to a `Component`, where `affords`, `supports` and `judge` can act on it. An unlock
//! is an *identity*, and identity is all it is. Storing it as a class cost every author a second, inert
//! file beside the component that did the real work, and bounded `grants()`'s picker at `Kind<Object>`
//! — the root of everything.
//!
//! # Identity is the id, never the name
//!
//! [`Unlock::id`] is generated once and never edited; `name` is an editable label. Renaming a row must
//! rewrite **zero** references, which is the same rule class paths follow: *the path is the id in a
//! table; a move rewrites one row.* ⚠ This is the one decision here that cannot be tightened later
//! without a migration, so `supersedes` refers by id and the lattice keys on id everywhere.
//!
//! # Ordering is declared, not computed
//!
//! `supersedes` says *"holding this satisfies a requirement for any of these"* — the ordering that lets
//! a Longshot answer a lock written for a Hookshot. It replaces an overridable `satisfied_by` hook that
//! sat on `Object` itself, where `MeshComponent` inherited an opinion about progression.
//!
//! Because it is **data rather than logic**, two things follow that a hook could never offer:
//!
//! * a **cycle is reportable** — see [`UnlockTable::build`], which refuses to construct one;
//! * the closure is computed **once**, so the solver's hot path stays a set membership test.
//!
//! # Why the closure is expanded at grant time
//!
//! [`UnlockTable::closure_of`] is applied when an unlock is *granted*, not when a rule is *evaluated*.
//! The sweep already does `held.extend(&granted)`, so extending with the closure leaves
//! `Rule::Has(c) => held.contains(c)` and its signature completely untouched. Threading the closure
//! into every rule evaluation instead would ripple through the solver, the softlock pass and every
//! caller, for an identical answer.
//!
//! # Determinism
//!
//! Rows are held in a `Vec` in file order and indexed by a `BTreeMap`, never a `HashMap`: the order
//! reaches the fingerprint, so it must be identical on every run and every target.

use crate::object::ObjectId;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// What obtaining a piece of content hands out.
///
/// ⚠ **A multimap, because one pickup may grant several separable atoms.** Super Metroid's Speed
/// Booster grants *run* and *shinespark*, and level designers gate them independently — collapsing them
/// into one atom would make every shinespark room reachable the moment any speed-block room is.
///
/// Values are **already closure-expanded** through [`UnlockTable::expand`], so every downstream rule
/// test stays a plain `held.contains(..)`.
pub type GrantMap = BTreeMap<ObjectId, BTreeSet<ObjectId>>;

/// One row of an unlock table — one atom of the progression lattice.
///
/// Carries no behaviour by construction. If you find yourself wanting to add a hook here, the thing
/// you want is a `Component`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unlock {
    /// Stable identity, generated once and never edited.
    ///
    /// ⚠ Everything keys on this. `name` is a label a developer may change freely.
    pub id: String,
    /// The display name, shown in pickers and traces.
    pub name: String,
    /// What this is, in the developer's words. Shown in the picker and in the trace.
    pub doc: String,
    /// Ids this row supersedes: *holding this satisfies a requirement for any of these.*
    ///
    /// ⚠ By **id**, never by name — so a rename rewrites one cell and breaks nothing.
    pub supersedes: Vec<String>,
}

impl Unlock {
    /// A row with no ordering, which is the common case.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Unlock {
            id: id.into(),
            name: name.into(),
            doc: String::new(),
            supersedes: Vec::new(),
        }
    }

    /// Declare that holding this satisfies a requirement for `other`.
    pub fn superseding(mut self, other: impl Into<String>) -> Self {
        self.supersedes.push(other.into());
        self
    }

    /// Prose for the picker and the trace.
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = doc.into();
        self
    }

    /// The lattice key. Derived from the **id**, so a rename never moves it.
    pub fn key(&self) -> ObjectId {
        ObjectId::derived("unlock", &self.id)
    }
}

/// What made a table unbuildable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableError {
    /// Two rows share an id. Identity would be ambiguous, so nothing can be keyed.
    DuplicateId(String),
    /// A `supersedes` entry names an id that is not in the table.
    UnknownSupersedes { row: String, missing: String },
    /// `supersedes` contains a cycle, reported with the ring so it can be broken.
    ///
    /// ⚠ A cycle means *"A satisfies a requirement for B, and B for A"*, which makes the two
    /// indistinguishable to every query while pretending to be ordered.
    Cycle(Vec<String>),
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableError::DuplicateId(id) => write!(f, "two rows share the id `{id}`"),
            TableError::UnknownSupersedes { row, missing } => {
                write!(
                    f,
                    "`{row}` supersedes `{missing}`, which is not in the table"
                )
            }
            TableError::Cycle(ring) => {
                write!(f, "supersedes cycle: {}", ring.join(" -> "))
            }
        }
    }
}

/// The project's progression vocabulary.
///
/// Built in memory here; the `.cvunlock` file that feeds it arrives at M14 beside `.cvcurve`, because
/// JSON parsing, asset resolution and content hashing are one piece of work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnlockTable {
    rows: Vec<Unlock>,
    by_id: BTreeMap<String, usize>,
    /// `id → every id it satisfies`, including itself. Computed once at build.
    closure: BTreeMap<String, BTreeSet<ObjectId>>,
}

impl UnlockTable {
    /// Build a table, validating identity and ordering.
    ///
    /// ⚠ Every failure here is a **build error** by design. A table that half-works would put a
    /// silently ungated lock into a world, and no seed would explain it.
    pub fn build(rows: Vec<Unlock>) -> Result<Self, TableError> {
        let mut by_id = BTreeMap::new();
        for (i, r) in rows.iter().enumerate() {
            if by_id.insert(r.id.clone(), i).is_some() {
                return Err(TableError::DuplicateId(r.id.clone()));
            }
        }
        for r in &rows {
            for s in &r.supersedes {
                if !by_id.contains_key(s) {
                    return Err(TableError::UnknownSupersedes {
                        row: r.id.clone(),
                        missing: s.clone(),
                    });
                }
            }
        }

        let mut table = UnlockTable {
            rows,
            by_id,
            closure: BTreeMap::new(),
        };
        table.closure = table.compute_closure()?;
        Ok(table)
    }

    /// Depth-first transitive closure with cycle detection, in declaration order.
    fn compute_closure(&self) -> Result<BTreeMap<String, BTreeSet<ObjectId>>, TableError> {
        #[derive(Clone, Copy, PartialEq)]
        enum Mark {
            Open,
            Done,
        }
        let mut mark: BTreeMap<&str, Mark> = BTreeMap::new();
        let mut out: BTreeMap<String, BTreeSet<ObjectId>> = BTreeMap::new();
        // An explicit stack rather than recursion: a pathological table should report a cycle, not
        // overflow the stack on the way to reporting it.
        for root in &self.rows {
            if mark.get(root.id.as_str()) == Some(&Mark::Done) {
                continue;
            }
            let mut path: Vec<&str> = vec![root.id.as_str()];
            let mut cursor: Vec<usize> = vec![0];
            mark.insert(root.id.as_str(), Mark::Open);
            while let Some(&id) = path.last() {
                let row = &self.rows[self.by_id[id]];
                let i = *cursor.last().expect("cursor tracks path");
                if i < row.supersedes.len() {
                    *cursor.last_mut().expect("cursor tracks path") += 1;
                    let next = row.supersedes[i].as_str();
                    match mark.get(next) {
                        Some(Mark::Open) => {
                            let start = path.iter().position(|p| *p == next).unwrap_or(0);
                            let mut ring: Vec<String> =
                                path[start..].iter().map(|s| s.to_string()).collect();
                            ring.push(next.to_string());
                            return Err(TableError::Cycle(ring));
                        }
                        Some(Mark::Done) => {}
                        None => {
                            mark.insert(next, Mark::Open);
                            path.push(next);
                            cursor.push(0);
                        }
                    }
                } else {
                    // Children complete: this row's closure is itself plus theirs.
                    let mut set = BTreeSet::new();
                    set.insert(row.key());
                    for s in &row.supersedes {
                        if let Some(child) = out.get(s) {
                            set.extend(child.iter().copied());
                        }
                    }
                    out.insert(row.id.clone(), set);
                    mark.insert(row.id.as_str(), Mark::Done);
                    path.pop();
                    cursor.pop();
                }
            }
        }
        Ok(out)
    }

    /// The rows, in declaration order.
    pub fn rows(&self) -> &[Unlock] {
        &self.rows
    }

    /// One row by its stable id.
    pub fn by_id(&self, id: &str) -> Option<&Unlock> {
        self.by_id.get(id).map(|i| &self.rows[*i])
    }

    /// One row by display name.
    ///
    /// ⚠ Convenience for authoring and tests only — **identity is the id**. Two rows may not share a
    /// name in practice, but nothing here depends on that.
    pub fn row(&self, name: &str) -> Option<&Unlock> {
        self.rows.iter().find(|r| r.name == name)
    }

    /// Everything holding `id` satisfies, including `id` itself.
    ///
    /// This is what a grant expands to. Empty for an unknown id, so a caller cannot accidentally
    /// widen the lattice by asking about something that is not in the table.
    pub fn closure_of(&self, id: &str) -> BTreeSet<ObjectId> {
        self.closure.get(id).cloned().unwrap_or_default()
    }

    /// Expand a set of granted ids into the lattice keys they satisfy.
    ///
    /// ⚠ **This is the whole integration point.** Call it where grants are collected, and every
    /// downstream rule test stays a plain `held.contains(..)`.
    pub fn expand<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> BTreeSet<ObjectId> {
        let mut out = BTreeSet::new();
        for id in ids {
            out.extend(self.closure_of(id));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ropes() -> UnlockTable {
        UnlockTable::build(vec![
            Unlock::new("u_pull", "PullToAnchor").with_doc("can tether to an anchor"),
            Unlock::new("u_long", "LongPullToAnchor").superseding("u_pull"),
            Unlock::new("u_hero", "HeroPullToAnchor").superseding("u_long"),
            Unlock::new("u_torch", "TorchOrder"),
        ])
        .expect("a valid table")
    }

    #[test]
    fn a_longer_rope_satisfies_a_shorter_requirement() {
        let t = ropes();
        let pull = t.by_id("u_pull").expect("u_pull").key();

        // The point of the whole mechanism: holding only the Longshot opens a Hookshot lock.
        assert!(t.expand(["u_long"]).contains(&pull));
        // And transitively, without `u_hero` naming `u_pull` at all.
        assert!(t.expand(["u_hero"]).contains(&pull));
        // Not the other way round — ordering is a partial order, not an equivalence.
        assert!(!t
            .expand(["u_pull"])
            .contains(&t.by_id("u_long").expect("u_long").key()));
    }

    #[test]
    fn an_unrelated_unlock_stays_unrelated() {
        let t = ropes();
        let torch = t.expand(["u_torch"]);
        assert_eq!(
            torch.len(),
            1,
            "a row with no supersedes satisfies only itself"
        );
        assert!(torch.contains(&t.by_id("u_torch").expect("u_torch").key()));
    }

    #[test]
    fn renaming_a_row_moves_no_key() {
        // The rule that cannot be tightened later: identity is the id.
        let before = Unlock::new("u_pull", "PullToAnchor").key();
        let after = Unlock::new("u_pull", "Grapple").key();
        assert_eq!(before, after, "a rename must not re-key the lattice");
    }

    #[test]
    fn a_cycle_is_a_build_error() {
        let err = UnlockTable::build(vec![
            Unlock::new("a", "A").superseding("b"),
            Unlock::new("b", "B").superseding("a"),
        ])
        .expect_err("a cycle must not build");
        match err {
            TableError::Cycle(ring) => {
                assert!(ring.len() >= 2, "the ring must name its members: {ring:?}")
            }
            other => panic!("expected a cycle, got {other:?}"),
        }
    }

    #[test]
    fn a_self_cycle_is_a_build_error() {
        assert!(matches!(
            UnlockTable::build(vec![Unlock::new("a", "A").superseding("a")]),
            Err(TableError::Cycle(_))
        ));
    }

    #[test]
    fn duplicate_ids_and_dangling_edges_are_build_errors() {
        assert_eq!(
            UnlockTable::build(vec![Unlock::new("a", "A"), Unlock::new("a", "Other")]),
            Err(TableError::DuplicateId("a".into()))
        );
        assert_eq!(
            UnlockTable::build(vec![Unlock::new("a", "A").superseding("ghost")]),
            Err(TableError::UnknownSupersedes {
                row: "a".into(),
                missing: "ghost".into()
            })
        );
    }

    #[test]
    fn a_diamond_resolves_once() {
        // Two paths to the same ancestor must not double-count or loop.
        let t = UnlockTable::build(vec![
            Unlock::new("base", "Base"),
            Unlock::new("l", "Left").superseding("base"),
            Unlock::new("r", "Right").superseding("base"),
            Unlock::new("top", "Top").superseding("l").superseding("r"),
        ])
        .expect("a diamond is legal");
        let top = t.expand(["top"]);
        assert_eq!(top.len(), 4, "base counted once: {top:?}");
    }

    #[test]
    fn expansion_is_order_independent_and_deterministic() {
        let t = ropes();
        assert_eq!(
            t.expand(["u_long", "u_torch"]),
            t.expand(["u_torch", "u_long"])
        );
    }
}
