//! **`CurveTableResource`** — named rows over one named domain axis.
//!
//! # One resource type where an engine usually has four
//!
//! A vector curve *is* three rows named `X`/`Y`/`Z`; a colour curve *is* four named `R`/`G`/`B`/`A`. So
//! `CurveFloat`, `CurveVector`, `CurveLinearColor` and `CurveTable` collapse into one thing: a table of
//! named rows. Nothing is lost, and a developer who adds a fourth row does not have to change the
//! table's *type*.
//!
//! ⚠ **Interpolation is declared per row, not per table.** UE fixes it per table only because a CSV has
//! one header row — JSON does not, and a table holding both a stepped unlock count and a smooth
//! difficulty ramp is an ordinary thing to want.
//!
//! # The domain is named, and that is the whole binding mechanism
//!
//! ⚠ A table declares `domain = "boss_count"` and whichever [`crate::axis::ProgressionAxis`] carries
//! that name supplies the number. **Binding by name rather than by passing an axis at each call site**
//! is what makes the omission *checkable*: a table naming a domain no axis provides is a load-time
//! diagnostic, not a silent zero.
//!
//! ⚠ **A `Curve` is one row and is not a resource.** It is a value — 2D point data — and appears in
//! graphs like any other. Being representable inside a schematic never determines a thing's own file
//! format.

use crate::path::AssetPath;
use crate::schedule::Curve;
use cv_determinism::math;
use std::collections::BTreeMap;
use std::fmt;

/// How a row reads between its keyframes.
///
/// ⚠ **Per row.** A table may hold a stepped count and a smooth ramp at once, and forcing one mode on
/// both would make the author fake the other with keyframes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Interpolation {
    /// Straight lines between keys.
    #[default]
    Linear,
    /// Hold the previous key's value until the next one.
    ///
    /// ⚠ The right mode for anything counted. *"Two bosses placed"* is not *"1.6 bosses"*, and a
    /// linear read of an integer axis invents values that never occur.
    Step,
    /// Smooth (smoothstep) between keys.
    Smooth,
}

/// One named row of a table.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The row's curve — the 2D point data.
    pub curve: Curve,
    /// How it reads between keys.
    pub interpolation: Interpolation,
}

impl Row {
    /// A linear row.
    pub fn linear(curve: Curve) -> Self {
        Row {
            curve,
            interpolation: Interpolation::Linear,
        }
    }

    /// A stepped row — for anything counted.
    pub fn stepped(curve: Curve) -> Self {
        Row {
            curve,
            interpolation: Interpolation::Step,
        }
    }

    /// A smoothed row.
    pub fn smooth(curve: Curve) -> Self {
        Row {
            curve,
            interpolation: Interpolation::Smooth,
        }
    }

    /// Read this row at `x`.
    ///
    /// ⚠ **Clamped outside the keyed range**, never extrapolated. Extrapolation invents values the
    /// author never drew, and the first place it shows up is the far end of a progression axis where
    /// nobody is looking.
    pub fn sample(&self, x: f64) -> f64 {
        let points = self.curve.points();
        match points.first() {
            None => 0.0,
            Some(first) if x <= first.0 => first.1,
            _ => {
                let last = points[points.len() - 1];
                if x >= last.0 {
                    return last.1;
                }
                for w in points.windows(2) {
                    let (x0, y0) = w[0];
                    let (x1, y1) = w[1];
                    if x <= x1 {
                        if x1 == x0 {
                            return y1;
                        }
                        let t = (x - x0) / (x1 - x0);
                        return match self.interpolation {
                            Interpolation::Linear => math::lerp(y0, y1, t),
                            Interpolation::Step => y0,
                            Interpolation::Smooth => math::lerp(y0, y1, t * t * (3.0 - 2.0 * t)),
                        };
                    }
                }
                last.1
            }
        }
    }
}

/// What can go wrong reading a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CurveError {
    /// The table has no row by that name.
    NoSuchRow { table: String, row: String },
    /// The table names a domain no axis provides.
    UnboundDomain { table: String, domain: String },
    /// Two rows registered under one name.
    DuplicateRow { table: String, row: String },
}

impl fmt::Display for CurveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CurveError::NoSuchRow { table, row } => write!(f, "{table} has no row {row:?}"),
            CurveError::UnboundDomain { table, domain } => write!(
                f,
                "{table} is read over {domain:?}, and no ProgressionAxis carries that name"
            ),
            CurveError::DuplicateRow { table, row } => {
                write!(f, "{table} declares two rows named {row:?}")
            }
        }
    }
}

impl std::error::Error for CurveError {}

/// An authored table: named rows over one named domain axis.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveTable {
    /// Where it came from, for diagnostics.
    path: AssetPath,
    /// **The axis name its rows are read at.**
    domain: String,
    rows: BTreeMap<String, Row>,
}

impl CurveTable {
    /// An empty table over a named domain.
    pub fn new(path: AssetPath, domain: impl Into<String>) -> Self {
        CurveTable {
            path,
            domain: domain.into(),
            rows: BTreeMap::new(),
        }
    }

    /// Add a row.
    pub fn row(mut self, name: &str, row: Row) -> Self {
        self.rows.insert(name.to_string(), row);
        self
    }

    /// Add a row, reporting a name collision instead of overwriting.
    pub fn declare(&mut self, name: &str, row: Row) -> Result<(), CurveError> {
        if self.rows.contains_key(name) {
            return Err(CurveError::DuplicateRow {
                table: self.path.to_string(),
                row: name.to_string(),
            });
        }
        self.rows.insert(name.to_string(), row);
        Ok(())
    }

    /// The file this came from.
    pub fn path(&self) -> &AssetPath {
        &self.path
    }

    /// The axis name its rows are read at.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Every row name, in order.
    ///
    /// ⚠ **What the editor's row dropdown is generated from**, which is what makes a mistyped row
    /// structurally impossible rather than a runtime miss.
    pub fn row_names(&self) -> impl Iterator<Item = &str> {
        self.rows.keys().map(String::as_str)
    }

    /// How many rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Is the table empty?
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// One row, by name.
    pub fn get(&self, row: &str) -> Option<&Row> {
        self.rows.get(row)
    }

    /// Read a row at `x`.
    ///
    /// ⚠ **Missing rows are an error rather than a zero.** A zero is a legal curve value, so returning
    /// one for *"that row does not exist"* would make a typo indistinguishable from a deliberate
    /// authoring choice.
    pub fn sample(&self, row: &str, x: f64) -> Result<f64, CurveError> {
        self.rows
            .get(row)
            .map(|r| r.sample(x))
            .ok_or_else(|| CurveError::NoSuchRow {
                table: self.path.to_string(),
                row: row.to_string(),
            })
    }

    /// Read several rows at one `x` — how a vector or colour curve is read.
    ///
    /// ⚠ **The reason this is one table type and not four.** `sample_many(["X","Y","Z"], t)` is a
    /// vector curve; the same call with `["R","G","B","A"]` is a colour curve; neither needs its own
    /// resource type.
    pub fn sample_many(&self, rows: &[&str], x: f64) -> Result<Vec<f64>, CurveError> {
        rows.iter().map(|r| self.sample(r, x)).collect()
    }
}

/// Every loaded curve table, by file path.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CurveBook {
    tables: BTreeMap<String, CurveTable>,
}

impl CurveBook {
    /// An empty book.
    pub fn new() -> Self {
        CurveBook::default()
    }

    /// Load a table.
    pub fn add(&mut self, table: CurveTable) {
        self.tables.insert(table.path.to_string(), table);
    }

    /// Load a table, chaining.
    pub fn with(mut self, table: CurveTable) -> Self {
        self.add(table);
        self
    }

    /// A table, by the asset path that referenced it.
    pub fn get(&self, path: &AssetPath) -> Option<&CurveTable> {
        self.tables.get(path.as_str())
    }

    /// How many tables are loaded.
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// Is the book empty?
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    /// Every loaded table, in path order.
    pub fn iter(&self) -> impl Iterator<Item = &CurveTable> {
        self.tables.values()
    }

    /// Every domain any loaded table reads over.
    ///
    /// ⚠ What the axis lint compares against: a domain here with no axis carrying that name is a
    /// table nobody can read.
    pub fn domains(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.tables.values().map(|t| t.domain()).collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(p: &str) -> AssetPath {
        AssetPath::new(p).unwrap()
    }

    fn progression() -> CurveTable {
        CurveTable::new(asset("/Content/Curves/progression.cvcurve"), "depth")
            // "linear at first, opening up the deeper you go".
            .row(
                "complexity",
                Row::linear(Curve::from_points([(0.0, 0.2), (0.5, 0.4), (1.0, 1.0)])),
            )
            .row(
                "boss_count",
                Row::stepped(Curve::from_points([(0.0, 0.0), (0.4, 1.0), (0.8, 2.0)])),
            )
    }

    // --- the shape of a table --------------------------------------------------------------------

    #[test]
    fn one_table_carries_several_rows_which_is_why_it_is_one_type() {
        // ⚠ A vector curve *is* three rows; a colour curve *is* four. Four resource types collapse
        // into one, and adding a row does not change the table's type.
        let vector = CurveTable::new(asset("/Content/Curves/wobble.cvcurve"), "time")
            .row(
                "X",
                Row::linear(Curve::from_points([(0.0, 0.0), (1.0, 3.0)])),
            )
            .row(
                "Y",
                Row::linear(Curve::from_points([(0.0, 1.0), (1.0, 1.0)])),
            )
            .row(
                "Z",
                Row::linear(Curve::from_points([(0.0, 0.0), (1.0, -2.0)])),
            );

        assert_eq!(
            vector.sample_many(&["X", "Y", "Z"], 0.5).unwrap(),
            vec![1.5, 1.0, -1.0]
        );
        assert_eq!(vector.len(), 3);
    }

    #[test]
    fn interpolation_is_per_row_because_a_table_may_hold_both_kinds() {
        // ⚠ *"Two bosses placed"* is not *"1.6 bosses"*. A linear read of a counted axis invents values
        // that never occur, and forcing one mode per table would make the author fake it with keys.
        let t = progression();
        assert_eq!(t.sample("boss_count", 0.6).unwrap(), 1.0, "stepped holds");
        let smooth = t.sample("complexity", 0.25).unwrap();
        assert!(
            smooth > 0.2 && smooth < 0.4,
            "linear interpolates, got {smooth}"
        );
    }

    #[test]
    fn linear_step_and_smooth_agree_at_the_keys_and_differ_between_them() {
        let pts = [(0.0, 0.0), (1.0, 10.0)];
        let l = Row::linear(Curve::from_points(pts));
        let s = Row::stepped(Curve::from_points(pts));
        let m = Row::smooth(Curve::from_points(pts));

        for r in [&l, &s, &m] {
            assert_eq!(r.sample(0.0), 0.0);
            assert_eq!(r.sample(1.0), 10.0);
        }
        assert_eq!(l.sample(0.5), 5.0);
        assert_eq!(s.sample(0.5), 0.0, "a step holds the previous value");
        assert_eq!(
            m.sample(0.5),
            5.0,
            "smoothstep is symmetric at the midpoint"
        );
        assert!(m.sample(0.25) < l.sample(0.25), "smooth eases in");
    }

    #[test]
    fn a_row_is_clamped_outside_its_keys_and_never_extrapolated() {
        // ⚠ Extrapolation invents values the author never drew, and the first place it shows is the
        // far end of an axis where nobody is looking.
        let t = progression();
        assert_eq!(t.sample("complexity", -5.0).unwrap(), 0.2);
        assert_eq!(t.sample("complexity", 99.0).unwrap(), 1.0);
    }

    // --- the domain is the binding ------------------------------------------------------------

    #[test]
    fn a_table_names_the_axis_it_is_read_over_rather_than_taking_one() {
        // ⚠ Binding by name is what makes the omission checkable. Passing an axis at each call site
        // would make a missing axis a *caller* bug rather than a load-time diagnostic.
        let t = progression();
        assert_eq!(t.domain(), "depth");

        let book = CurveBook::new().with(t).with(CurveTable::new(
            asset("/Content/Curves/boss.cvcurve"),
            "boss_count",
        ));
        assert_eq!(book.domains(), vec!["boss_count", "depth"]);
    }

    // --- errors say what is wrong ----------------------------------------------------------------

    #[test]
    fn a_missing_row_is_an_error_and_not_a_zero() {
        // ⚠ Zero is a legal curve value, so returning it for *"no such row"* makes a typo
        // indistinguishable from a deliberate authoring choice.
        let t = progression();
        assert!(matches!(
            t.sample("complexty", 0.5),
            Err(CurveError::NoSuchRow { .. })
        ));
        assert_eq!(t.sample("complexity", 0.0).unwrap(), 0.2);
    }

    #[test]
    fn declaring_a_row_twice_is_refused_rather_than_overwriting() {
        let mut t = progression();
        assert!(matches!(
            t.declare("complexity", Row::linear(Curve::constant(9.0))),
            Err(CurveError::DuplicateRow { .. })
        ));
        assert_eq!(t.sample("complexity", 1.0).unwrap(), 1.0, "unchanged");
    }

    #[test]
    fn row_names_are_ordered_so_the_editor_dropdown_is_stable() {
        // ⚠ The dropdown is generated from this, which is what makes a mistyped row structurally
        // impossible rather than a runtime miss. An unstable order would reshuffle it every load.
        let t = progression();
        assert_eq!(
            t.row_names().collect::<Vec<_>>(),
            vec!["boss_count", "complexity"]
        );
    }

    #[test]
    fn an_empty_row_reads_as_zero_rather_than_panicking() {
        let r = Row::linear(Curve::from_points([]));
        assert_eq!(r.sample(0.5), 0.0);
    }

    #[test]
    fn a_book_finds_a_table_by_the_asset_path_that_referenced_it() {
        let book = CurveBook::new().with(progression());
        let p = asset("/Content/Curves/progression.cvcurve");
        assert!(book.get(&p).is_some());
        assert!(book.get(&asset("/Content/Curves/nope.cvcurve")).is_none());
    }
}
