//! **`.cvcurve` and `.cvunlock`** — the two data resources, both JSON.
//!
//! ⚠ **JSON, not CVB, and the reason is the same for both: there are no nodes to notate.** A curve
//! table's editor is a 2D plot; an unlock table's is a row list. Writing them in the block notation
//! would buy a shared parser and cost every tool that wants to read them without one.
//!
//! # Interpolation is per row, not per table
//!
//! ⚠ **UE fixes it per table only because a CSV has one header row.** JSON has no such constraint, and
//! one table wanting a stepped `tier` row beside a smooth `difficulty` row is ordinary. A CSV importer
//! may exist as a **converter**; flattening to a single interpolation would be the CSV's limitation
//! rather than the format's.
//!
//! # `supersedes` refers by id, and a cycle is a build error
//!
//! ⚠ **By id, never by name**, so renaming an unlock rewrites one cell and breaks nothing. And the
//! closure is taken **once, at load** — which is what lets a cycle be a *build* error. An overridable
//! matcher hook could never have said that: it would have to answer *"does holding A satisfy B?"* one
//! pair at a time, with no vantage point from which the loop is visible.

use crate::json::{parse, Json};
use cv_core::curve::{CurveTable, Interpolation, Row};
use cv_core::path::AssetPath;
use cv_core::schedule::Curve;
use cv_core::unlock::{TableError, Unlock, UnlockTable};
use std::fmt;

/// Why an asset did not load.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    /// The JSON itself did not read.
    Malformed { detail: String },
    /// A required member is missing.
    Missing { member: String },
    /// A member is present with the wrong shape.
    WrongShape {
        member: String,
        expected: &'static str,
        found: &'static str,
    },
    /// The file declares a version this build does not read.
    ///
    /// ⚠ **`version` is declared and checked, never sniffed.** A loader that inferred the version from
    /// the shape would accept a future file that happens to look like a current one.
    UnknownVersion { found: f64 },
    /// An interpolation name the vocabulary does not list.
    UnknownInterpolation { written: String },
    /// `supersedes` names an id the table does not contain.
    DanglingSupersedes { row: String, missing: String },
    /// ⚠ **A cycle in `supersedes`** — A satisfies B satisfies A.
    SupersedesCycle { ids: Vec<String> },
    /// Two rows share an id.
    DuplicateId { id: String },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Malformed { detail } => write!(f, "not readable JSON: {detail}"),
            LoadError::Missing { member } => write!(f, "no {member}"),
            LoadError::WrongShape {
                member,
                expected,
                found,
            } => write!(f, "{member} should be {expected}, found {found}"),
            LoadError::UnknownVersion { found } => write!(
                f,
                "version {found} is not one this build reads — the version is declared, never sniffed"
            ),
            LoadError::UnknownInterpolation { written } => write!(
                f,
                "{written:?} is not an interpolation — the set is LINEAR, STEP and CUBIC"
            ),
            LoadError::DanglingSupersedes { row, missing } => write!(
                f,
                "{row} supersedes {missing}, which is not in this table — supersedes refers by id"
            ),
            LoadError::SupersedesCycle { ids } => write!(
                f,
                "a cycle in supersedes: {} — holding each would satisfy the others, which has no \
                 ordering, so it is a build error rather than a question asked one pair at a time",
                ids.join(" → ")
            ),
            LoadError::DuplicateId { id } => write!(f, "two rows share the id {id}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// The only `version` these loaders read.
pub const VERSION: f64 = 1.0;

fn check_version(doc: &Json) -> Result<(), LoadError> {
    let Some(v) = doc.get("version") else {
        return Err(LoadError::Missing {
            member: "version".into(),
        });
    };
    match v.as_f64() {
        Some(n) if n == VERSION => Ok(()),
        Some(n) => Err(LoadError::UnknownVersion { found: n }),
        None => Err(LoadError::WrongShape {
            member: "version".into(),
            expected: "a number",
            found: v.kind(),
        }),
    }
}

fn text(doc: &Json, member: &str) -> Result<String, LoadError> {
    let Some(v) = doc.get(member) else {
        return Err(LoadError::Missing {
            member: member.into(),
        });
    };
    v.as_str().map(str::to_string).ok_or(LoadError::WrongShape {
        member: member.into(),
        expected: "a string",
        found: v.kind(),
    })
}

/// A loaded curve table, plus the label its editor puts on the vertical axis.
///
/// ⚠ **`y_label` lives here rather than on [`CurveTable`]** because it is an *editor* fact: nothing in
/// the generator reads it, and putting a presentation string on a core type would make every consumer
/// carry a field only one of them uses.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedCurves {
    /// The table itself.
    pub table: CurveTable,
    /// What the curve editor labels the vertical axis. Empty when the file did not say.
    pub y_label: String,
}

/// Read a `.cvcurve`.
pub fn load_curves(path: AssetPath, src: &str) -> Result<LoadedCurves, LoadError> {
    let doc = parse(src).map_err(|e| LoadError::Malformed {
        detail: e.to_string(),
    })?;
    check_version(&doc)?;
    let domain = text(&doc, "domain")?;
    let y_label = doc
        .get("y_label")
        .and_then(Json::as_str)
        .unwrap_or_default()
        .to_string();

    let Some(rows) = doc.get("rows") else {
        return Err(LoadError::Missing {
            member: "rows".into(),
        });
    };
    let Some(entries) = rows.as_object() else {
        return Err(LoadError::WrongShape {
            member: "rows".into(),
            expected: "an object",
            found: rows.kind(),
        });
    };

    let mut table = CurveTable::new(path, domain);
    for (name, row) in entries {
        let interpolation = match row.get("interpolation").and_then(Json::as_str) {
            None => Interpolation::default(),
            Some(written) => interpolation_of(written)?,
        };
        let Some(points) = row.get("points").and_then(Json::as_array) else {
            return Err(LoadError::Missing {
                member: format!("rows.{name}.points"),
            });
        };
        let mut keys: Vec<(f64, f64)> = Vec::with_capacity(points.len());
        for p in points {
            let Some(pair) = p.as_array() else {
                return Err(LoadError::WrongShape {
                    member: format!("rows.{name}.points"),
                    expected: "an array of [x, y] pairs",
                    found: p.kind(),
                });
            };
            let (Some(x), Some(y)) = (
                pair.first().and_then(Json::as_f64),
                pair.get(1).and_then(Json::as_f64),
            ) else {
                return Err(LoadError::WrongShape {
                    member: format!("rows.{name}.points"),
                    expected: "an array of [x, y] pairs",
                    found: "something else",
                });
            };
            keys.push((x, y));
        }
        table
            .declare(
                name,
                Row {
                    curve: Curve::from_points(keys),
                    interpolation,
                },
            )
            .ok();
    }

    Ok(LoadedCurves { table, y_label })
}

/// ⚠ **`CUBIC` in the file is [`Interpolation::Smooth`] in the core.**
///
/// A recorded divergence rather than a rename in either direction: the file uses the word a curve
/// editor's users expect, and the core uses the word that describes what it does — smoothstep, which is
/// not a cubic spline and would be a lie to call one.
fn interpolation_of(written: &str) -> Result<Interpolation, LoadError> {
    Ok(match written {
        "LINEAR" => Interpolation::Linear,
        "STEP" => Interpolation::Step,
        "CUBIC" | "SMOOTH" => Interpolation::Smooth,
        _ => {
            return Err(LoadError::UnknownInterpolation {
                written: written.to_string(),
            })
        }
    })
}

/// Read a `.cvunlock`.
///
/// ⚠ **`supersedes` is validated before the table is built.** A dangling id or a cycle has to stop the
/// load rather than produce a table whose closure is wrong: the closure is taken once and everything
/// downstream trusts it.
pub fn load_unlocks(src: &str) -> Result<UnlockTable, LoadError> {
    // WARN **The ordering rules are the core's, and this loader does not restate them.**
    // `UnlockTable::build` refuses a duplicate id, a dangling `supersedes` and a cycle.
    UnlockTable::build(unlock_rows(src)?).map_err(LoadError::from)
}

/// The rows of a `.cvunlock`, **without enforcing the lattice**.
///
/// WARN **Separated because a view and an engine want different things from the same file.** The
/// engine must refuse a cycle before building — the closure is taken once and everything downstream
/// trusts it. A *table view* must show the developer the cycle, on the rows that form it, which it
/// cannot do if the read refused.
///
/// RARR **One parser, two callers.** A second reader here would be two places that must agree about
/// what a `.cvunlock` is.
fn unlock_rows(src: &str) -> Result<Vec<Unlock>, LoadError> {
    let doc = parse(src).map_err(|e| LoadError::Malformed {
        detail: e.to_string(),
    })?;
    check_version(&doc)?;

    let Some(rows) = doc.get("unlocks").and_then(Json::as_array) else {
        return Err(LoadError::Missing {
            member: "unlocks".into(),
        });
    };

    let mut parsed: Vec<Unlock> = Vec::with_capacity(rows.len());
    for row in rows {
        let id = text(row, "id")?;
        let supersedes = match row.get("supersedes") {
            None => Vec::new(),
            Some(v) => {
                let Some(items) = v.as_array() else {
                    return Err(LoadError::WrongShape {
                        member: "supersedes".into(),
                        expected: "an array of ids",
                        found: v.kind(),
                    });
                };
                items
                    .iter()
                    .filter_map(|i| i.as_str().map(str::to_string))
                    .collect()
            }
        };
        parsed.push(Unlock {
            id,
            name: text(row, "name")?,
            doc: row
                .get("doc")
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_string(),
            supersedes,
        });
    }

    Ok(parsed)
}

/// What a `.cvunlock` holds, and what is wrong with it.
///
/// ⚠ **Rows *and* a fault, not one or the other.** A table view that received only an error could
/// show a message and no table — leaving the developer to find the cycle in a file by reading it,
/// which is what the view exists to replace.
pub struct InspectedUnlocks {
    /// Every row, in file order.
    pub rows: Vec<Unlock>,
    /// What stops this table building, if anything.
    pub fault: Option<TableError>,
}

/// Read a `.cvunlock` **for a view**: the rows, plus whatever stops them building.
///
/// ▶ **The fault names its rows.** `Cycle` carries the ids in it, `UnknownSupersedes` the row and
/// the id it could not find — enough to mark the offending rows in a table rather than print a
/// sentence beside it.
pub fn inspect_unlocks(src: &str) -> Result<InspectedUnlocks, LoadError> {
    let rows = unlock_rows(src)?;
    let fault = UnlockTable::build(rows.clone()).err();
    Ok(InspectedUnlocks { rows, fault })
}

impl From<TableError> for LoadError {
    fn from(e: TableError) -> Self {
        match e {
            TableError::DuplicateId(id) => LoadError::DuplicateId { id },
            TableError::UnknownSupersedes { row, missing } => {
                LoadError::DanglingSupersedes { row, missing }
            }
            TableError::Cycle(ids) => LoadError::SupersedesCycle { ids },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> AssetPath {
        AssetPath::new("/Content/Curves/progression.cvcurve").unwrap()
    }

    const CURVES: &str = r#"
    { "version": 1,
      "domain":  "depth",
      "y_label": "multiplier",
      "rows": {
        "complexity":     { "interpolation": "CUBIC",  "points": [[0.0,1.0],[0.5,3.0],[1.0,6.0]] },
        "hazard_density": { "interpolation": "LINEAR", "points": [[0.0,0.1],[1.0,0.8]] },
        "tier":           { "interpolation": "STEP",   "points": [[0.0,1.0],[0.5,2.0]] }
      } }"#;

    const UNLOCKS: &str = r#"
    { "version": 1,
      "unlocks": [
        { "id": "u_7f3a91", "name": "PullToAnchor",     "doc": "can tether to an anchor",
          "supersedes": [] },
        { "id": "u_2c14e8", "name": "LongPullToAnchor", "doc": "a longer tether",
          "supersedes": ["u_7f3a91"] },
        { "id": "u_be0d52", "name": "TorchOrder",       "doc": "knows the four-torch order",
          "supersedes": [] }
      ] }"#;

    #[test]
    fn the_design_documents_own_curve_table_loads() {
        let got = load_curves(path(), CURVES).unwrap();
        assert_eq!(got.table.domain(), "depth");
        assert_eq!(
            got.y_label, "multiplier",
            "P05: the editor labels the vertical"
        );
        assert_eq!(got.table.len(), 3);
    }

    #[test]
    fn interpolation_is_per_row_and_not_per_table() {
        // ⚠ One table with a stepped row beside a smooth one is ordinary; UE fixes it per table only
        // because a CSV has one header row.
        let got = load_curves(path(), CURVES).unwrap();
        assert_eq!(
            got.table.get("complexity").unwrap().interpolation,
            Interpolation::Smooth
        );
        assert_eq!(
            got.table.get("hazard_density").unwrap().interpolation,
            Interpolation::Linear
        );
        assert_eq!(
            got.table.get("tier").unwrap().interpolation,
            Interpolation::Step
        );
    }

    #[test]
    fn a_stepped_row_never_invents_a_value_between_its_keys() {
        // ⚠ "Two bosses placed" is not "1.6 bosses".
        let got = load_curves(path(), CURVES).unwrap();
        assert_eq!(got.table.sample("tier", 0.4).unwrap(), 1.0);
        assert_eq!(got.table.sample("tier", 0.9).unwrap(), 2.0);
    }

    #[test]
    fn an_unknown_interpolation_names_the_set() {
        let bad = CURVES.replace("\"CUBIC\"", "\"BEZIER\"");
        let err = load_curves(path(), &bad).unwrap_err();
        assert_eq!(
            err,
            LoadError::UnknownInterpolation {
                written: "BEZIER".into()
            }
        );
        assert!(err.to_string().contains("LINEAR, STEP and CUBIC"));
    }

    #[test]
    fn a_row_with_no_interpolation_takes_the_default_rather_than_failing() {
        let got = load_curves(
            path(),
            r#"{"version":1,"domain":"d","rows":{"a":{"points":[[0.0,1.0]]}}}"#,
        )
        .unwrap();
        assert_eq!(
            got.table.get("a").unwrap().interpolation,
            Interpolation::Linear
        );
    }

    #[test]
    fn the_version_is_declared_and_never_sniffed() {
        // ⚠ A loader that inferred it would accept a future file that happens to look current.
        assert_eq!(
            load_curves(path(), r#"{"domain":"d","rows":{}}"#).unwrap_err(),
            LoadError::Missing {
                member: "version".into()
            }
        );
        let err = load_curves(path(), r#"{"version":2,"domain":"d","rows":{}}"#).unwrap_err();
        assert_eq!(err, LoadError::UnknownVersion { found: 2.0 });
        assert!(err.to_string().contains("never sniffed"));
    }

    #[test]
    fn the_design_documents_own_unlock_table_loads_with_its_edges() {
        let table = load_unlocks(UNLOCKS).unwrap();
        assert_eq!(table.rows().len(), 3);
        let long = table.by_id("u_2c14e8").expect("the upgrade row");
        assert_eq!(long.name, "LongPullToAnchor");
        assert_eq!(long.supersedes, vec!["u_7f3a91".to_string()]);
    }

    #[test]
    fn holding_an_upgrade_satisfies_what_it_supersedes() {
        // ⚠ A door written for a PullToAnchor opens for a LongPullToAnchor without knowing it exists.
        let table = load_unlocks(UNLOCKS).unwrap();
        let satisfied = table.closure_of("u_2c14e8");
        let base = table.by_id("u_7f3a91").expect("the base row").key();
        assert!(
            satisfied.contains(&base),
            "the closure must carry what the upgrade supersedes"
        );
    }

    #[test]
    fn supersedes_refers_by_id_so_a_rename_breaks_nothing() {
        // ⚠ Renaming rewrites one cell.
        let renamed = UNLOCKS.replace("\"PullToAnchor\"", "\"AnchorTether\"");
        let table = load_unlocks(&renamed).unwrap();
        assert_eq!(table.by_id("u_7f3a91").unwrap().name, "AnchorTether");
        assert_eq!(
            table.by_id("u_2c14e8").unwrap().supersedes,
            vec!["u_7f3a91".to_string()],
            "the edge still points at the same id"
        );
    }

    #[test]
    fn a_supersedes_cycle_is_a_build_error_that_names_the_loop() {
        // ⚠ "There is a cycle" against forty rows is a search; "A → B → A" is the two cells to edit.
        let cyclic = r#"
        { "version": 1,
          "unlocks": [
            { "id": "a", "name": "A", "supersedes": ["b"] },
            { "id": "b", "name": "B", "supersedes": ["a"] }
          ] }"#;
        let err = load_unlocks(cyclic).unwrap_err();
        let LoadError::SupersedesCycle { ids } = &err else {
            panic!("expected a cycle, got {err:?}");
        };
        assert!(ids.contains(&"a".to_string()) && ids.contains(&"b".to_string()));
        assert!(err.to_string().contains("build error"));
    }

    #[test]
    fn a_longer_cycle_is_found_too() {
        let cyclic = r#"
        { "version": 1,
          "unlocks": [
            { "id": "a", "name": "A", "supersedes": ["b"] },
            { "id": "b", "name": "B", "supersedes": ["c"] },
            { "id": "c", "name": "C", "supersedes": ["a"] }
          ] }"#;
        assert!(matches!(
            load_unlocks(cyclic).unwrap_err(),
            LoadError::SupersedesCycle { .. }
        ));
    }

    #[test]
    fn a_self_supersede_is_a_cycle_of_one() {
        let cyclic =
            r#"{ "version": 1, "unlocks": [ { "id": "a", "name": "A", "supersedes": ["a"] } ] }"#;
        assert!(matches!(
            load_unlocks(cyclic).unwrap_err(),
            LoadError::SupersedesCycle { .. }
        ));
    }

    #[test]
    fn a_diamond_is_not_a_cycle() {
        // ⚠ Two paths to the same ancestor is ordinary; a checker that called it a cycle would refuse
        // every real upgrade tree.
        let diamond = r#"
        { "version": 1,
          "unlocks": [
            { "id": "base", "name": "Base", "supersedes": [] },
            { "id": "l",    "name": "L",    "supersedes": ["base"] },
            { "id": "r",    "name": "R",    "supersedes": ["base"] },
            { "id": "top",  "name": "Top",  "supersedes": ["l","r"] }
          ] }"#;
        assert_eq!(load_unlocks(diamond).unwrap().rows().len(), 4);
    }

    #[test]
    fn a_dangling_supersedes_names_both_ends() {
        let bad = r#"{ "version": 1, "unlocks": [ { "id": "a", "name": "A", "supersedes": ["ghost"] } ] }"#;
        assert_eq!(
            load_unlocks(bad).unwrap_err(),
            LoadError::DanglingSupersedes {
                row: "a".into(),
                missing: "ghost".into()
            }
        );
    }

    #[test]
    fn two_rows_sharing_an_id_are_refused() {
        let bad = r#"
        { "version": 1,
          "unlocks": [ { "id": "a", "name": "A" }, { "id": "a", "name": "B" } ] }"#;
        assert_eq!(
            load_unlocks(bad).unwrap_err(),
            LoadError::DuplicateId { id: "a".into() }
        );
    }

    #[test]
    fn a_row_may_omit_its_doc_and_its_supersedes() {
        let table = load_unlocks(r#"{"version":1,"unlocks":[{"id":"a","name":"A"}]}"#).unwrap();
        let row = table.by_id("a").unwrap();
        assert!(row.doc.is_empty());
        assert!(row.supersedes.is_empty());
    }

    #[test]
    fn malformed_json_is_reported_as_such_rather_than_as_a_missing_member() {
        let err = load_unlocks("{ not json").unwrap_err();
        assert!(matches!(err, LoadError::Malformed { .. }));
    }
}
