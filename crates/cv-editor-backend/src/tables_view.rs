//! **The unlock-table and Dials views** — one edits a vocabulary, the other turns knobs.
//!
//! # Renaming edits `name` only, and the view says so
//!
//! ⚠ **`id` is fixed at creation and `name` is a label.** That is what makes a rename rewrite one cell
//! and break nothing — and the view must make it **visible rather than implied**, because a developer
//! who believes the name is the identity will avoid renaming, which is the opposite of the property.
//!
//! # A `supersedes` cycle is shown in the table
//!
//! ⚠ **Not deferred to a build error.** The developer is looking at the two rows that make the loop; a
//! message that arrived at the next build would arrive somewhere else entirely.
//!
//! # The Dials view turns knobs and creates none
//!
//! ⚠ **It is `project.dials` and nothing else.** The panel calls the same `list` / `get` / `set` a host
//! calls, because a shipped game must have the same fine-grained control the editor has — **the editor
//! is not allowed a private channel**. Creation is the `DIALS` section on the owner.

use cv_bindings::{DialKind, DialMeta, DialSource, Dials};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// One row of the unlock table, as the view renders it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlockRow {
    /// ⚠ **Fixed at creation.** The view renders it read-only, which is how *"renaming breaks nothing"*
    /// becomes something a developer can see rather than something they are told.
    pub id: String,
    /// The editable label.
    pub name: String,
    /// The developer's words.
    pub doc: String,
    /// Ids this supersedes, picked from the same table.
    pub supersedes: Vec<String>,
}

/// Whether a cell may be edited.
///
/// ⚠ **A property of the column, not of the row.** A per-row rule would eventually let one row's id be
/// edited, and the id is the one thing nothing may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Editable {
    /// Read-only, and shown as such.
    Never,
    /// Free text.
    Text,
    /// Picked from this table's own rows.
    PickFromTable,
}

/// The columns the unlock view renders.
pub const UNLOCK_COLUMNS: [(&str, Editable); 4] = [
    // ⚠ Read-only, and visibly so.
    ("id", Editable::Never),
    ("name", Editable::Text),
    ("doc", Editable::Text),
    // ⚠ Picked from the same table, never typed — a typed id is a dangling reference waiting to happen.
    ("supersedes", Editable::PickFromTable),
];

/// What the unlock view found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableFinding {
    /// ⚠ **A cycle, shown on the rows that make it.**
    Cycle { ids: Vec<String> },
    /// A `supersedes` naming an id the table does not have.
    Dangling { row: String, missing: String },
    /// Two rows sharing an id.
    DuplicateId { id: String },
    /// Two rows sharing a display name.
    ///
    /// ⚠ **A warning, not an error.** Names are labels, so two rows may legitimately share one — but a
    /// picker showing two identical entries is unusable, and the developer should know.
    DuplicateName { name: String },
}

impl TableFinding {
    /// Does this stop a build?
    pub fn blocks(&self) -> bool {
        !matches!(self, TableFinding::DuplicateName { .. })
    }
}

impl fmt::Display for TableFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableFinding::Cycle { ids } => write!(
                f,
                "these rows supersede each other in a loop: {} — holding any would satisfy the others, \
                 which has no ordering",
                ids.join(" → ")
            ),
            TableFinding::Dangling { row, missing } => {
                write!(f, "{row} supersedes {missing}, which is not in this table")
            }
            TableFinding::DuplicateId { id } => write!(f, "two rows share the id {id}"),
            TableFinding::DuplicateName { name } => write!(
                f,
                "two rows are both called {name:?} — legal, since a name is a label, but a picker \
                 showing two identical entries is unusable"
            ),
        }
    }
}

/// The unlock table, as the view holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnlockView {
    rows: Vec<UnlockRow>,
}

impl UnlockView {
    /// An empty table.
    pub fn new() -> Self {
        UnlockView::default()
    }

    /// Add a row.
    ///
    /// ⚠ **The id is supplied at creation and never again.** A method that could change it would be the
    /// affordance this design spends a section removing.
    pub fn add(&mut self, id: &str, name: &str, doc: &str) -> &mut Self {
        self.rows.push(UnlockRow {
            id: id.into(),
            name: name.into(),
            doc: doc.into(),
            supersedes: Vec::new(),
        });
        self
    }

    /// Rename a row.
    ///
    /// ⚠ **Edits `name` only.** Every reference is by id, so nothing else moves — which is the whole
    /// reason the two fields are separate.
    pub fn rename(&mut self, id: &str, name: &str) -> bool {
        match self.rows.iter_mut().find(|r| r.id == id) {
            Some(row) => {
                row.name = name.into();
                true
            }
            None => false,
        }
    }

    /// Make one row supersede another.
    pub fn supersede(&mut self, id: &str, base: &str) -> bool {
        match self.rows.iter_mut().find(|r| r.id == id) {
            Some(row) => {
                if !row.supersedes.iter().any(|s| s == base) {
                    row.supersedes.push(base.into());
                }
                true
            }
            None => false,
        }
    }

    /// Every row.
    pub fn rows(&self) -> &[UnlockRow] {
        &self.rows
    }

    /// What a `supersedes` picker offers for one row.
    ///
    /// ⚠ **Every other row, and never itself.** Offering itself would let a developer author the
    /// smallest possible cycle with one click.
    pub fn supersedes_options(&self, id: &str) -> Vec<&str> {
        self.rows
            .iter()
            .filter(|r| r.id != id)
            .map(|r| r.id.as_str())
            .collect()
    }

    /// Check the table, for drawing on it.
    pub fn check(&self) -> Vec<TableFinding> {
        let mut out = Vec::new();
        let mut ids: BTreeSet<&str> = BTreeSet::new();
        for r in &self.rows {
            if !ids.insert(r.id.as_str()) {
                out.push(TableFinding::DuplicateId { id: r.id.clone() });
            }
        }
        let mut names: BTreeMap<&str, usize> = BTreeMap::new();
        for r in &self.rows {
            *names.entry(r.name.as_str()).or_default() += 1;
        }
        for (name, count) in names {
            if count > 1 {
                out.push(TableFinding::DuplicateName { name: name.into() });
            }
        }
        for r in &self.rows {
            for s in &r.supersedes {
                if !ids.contains(s.as_str()) {
                    out.push(TableFinding::Dangling {
                        row: r.id.clone(),
                        missing: s.clone(),
                    });
                }
            }
        }
        if let Some(ids) = self.find_cycle() {
            out.push(TableFinding::Cycle { ids });
        }
        out
    }

    fn find_cycle(&self) -> Option<Vec<String>> {
        let edges: BTreeMap<&str, &[String]> = self
            .rows
            .iter()
            .map(|r| (r.id.as_str(), r.supersedes.as_slice()))
            .collect();
        let mut done: BTreeSet<&str> = BTreeSet::new();
        for start in self.rows.iter().map(|r| r.id.as_str()) {
            let mut path = Vec::new();
            let mut on_path = BTreeSet::new();
            if let Some(found) = walk(start, &edges, &mut path, &mut on_path, &mut done) {
                return Some(found);
            }
        }
        None
    }
}

fn walk<'a>(
    at: &'a str,
    edges: &BTreeMap<&'a str, &'a [String]>,
    path: &mut Vec<&'a str>,
    on_path: &mut BTreeSet<&'a str>,
    done: &mut BTreeSet<&'a str>,
) -> Option<Vec<String>> {
    if on_path.contains(at) {
        let from = path.iter().position(|p| *p == at).unwrap_or(0);
        let mut cycle: Vec<String> = path[from..].iter().map(|s| (*s).to_string()).collect();
        cycle.push(at.to_string());
        return Some(cycle);
    }
    if done.contains(at) {
        return None;
    }
    path.push(at);
    on_path.insert(at);
    for next in edges.get(at).copied().unwrap_or(&[]) {
        if let Some(found) = walk(next.as_str(), edges, path, on_path, done) {
            return Some(found);
        }
    }
    on_path.remove(at);
    path.pop();
    done.insert(at);
    None
}

/// How a dial's value is drawn in the Dials view.
///
/// ⚠ **A curve is a thumbnail, not a number.** There is no number to show — a curve-valued dial has a
/// *shape*, and rendering its first key would be showing one point of it as though it were the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rendering {
    /// A number field.
    Number,
    /// A pair of numbers.
    Pair,
    /// A dropdown.
    Choice,
    /// ⚠ A thumbnail of the curve's shape.
    Thumbnail,
}

/// One row of the standalone Dials view.
#[derive(Clone, Debug, PartialEq)]
pub struct DialRow {
    /// `<ClassName>.<DialName>`.
    pub id: String,
    /// The scope this row is grouped under.
    ///
    /// ⚠ **Grouped by owning *scope* then owner**, because a developer looking for a dial knows *where*
    /// in the world it applies before they know which class declared it.
    pub scope: String,
    /// The owner within that scope.
    pub owner: String,
    /// The developer's words.
    pub doc: String,
    /// Is it overridden?
    pub overridden: bool,
    /// ⚠ **Which scope supplied the value** — because with a scoped override in play, the number in the
    /// panel is not necessarily the number a given room uses.
    pub supplied_by: String,
    /// How to draw it.
    pub rendering: Rendering,
}

/// The standalone Dials view.
///
/// ⚠ **It holds no state of its own.** Every row is derived from `project.dials`, so the panel and a
/// host cannot disagree about what a dial is set to — which is what *"no private channel"* means in
/// practice rather than in principle.
pub struct DialsView;

impl DialsView {
    /// Every row, grouped and sorted.
    pub fn rows(dials: &Dials) -> Vec<DialRow> {
        let mut out: Vec<DialRow> = dials.list().into_iter().map(Self::row).collect();
        // ⚠ Scope, then owner, then id — the order the panel groups in.
        out.sort_by(|a, b| (&a.scope, &a.owner, &a.id).cmp(&(&b.scope, &b.owner, &b.id)));
        out
    }

    fn row(meta: &DialMeta) -> DialRow {
        DialRow {
            id: meta.id.clone(),
            scope: meta.scope.clone().unwrap_or_else(|| "World".into()),
            owner: meta.owner.clone(),
            doc: meta.doc.clone(),
            overridden: meta.source != DialSource::Authored,
            supplied_by: match meta.source {
                DialSource::Authored => "content".into(),
                DialSource::Host => "host".into(),
                DialSource::Scoped => meta
                    .scope
                    .clone()
                    .map(|s| format!("host, for {s}"))
                    .unwrap_or_else(|| "host".into()),
            },
            rendering: match meta.kind {
                DialKind::Number => Rendering::Number,
                DialKind::Range | DialKind::Adaptive => Rendering::Pair,
                DialKind::Enum => Rendering::Choice,
                DialKind::Curve | DialKind::Table => Rendering::Thumbnail,
            },
        }
    }

    /// Rows whose id, owner or doc matches a search.
    ///
    /// ⚠ **The doc is searched too.** A developer who remembers *"the one about rope length"* and not
    /// its id is the ordinary case, and a search over ids alone would fail them.
    pub fn search<'a>(rows: &'a [DialRow], query: &str) -> Vec<&'a DialRow> {
        let q = query.to_ascii_lowercase();
        rows.iter()
            .filter(|r| {
                r.id.to_ascii_lowercase().contains(&q)
                    || r.owner.to_ascii_lowercase().contains(&q)
                    || r.doc.to_ascii_lowercase().contains(&q)
            })
            .collect()
    }
}

/// **The floor slider** — shared state, not a per-view widget.
///
/// ⚠ **It lives above the views from the start rather than being retrofitted.** The skeleton view links
/// to it, so a slider owned by one view would have to be reached into by another — and the version of
/// that which works is the one where neither owns it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FloorSlider {
    /// Which floor is shown.
    pub floor: u32,
    /// How many there are.
    pub floors: u32,
}

impl FloorSlider {
    /// A slider over a scope with this many floors.
    pub fn over(floors: u32) -> Self {
        FloorSlider { floor: 0, floors }
    }

    /// Move it, clamped.
    ///
    /// ⚠ **Clamped rather than refused.** A slider is dragged, and a drag past the end is an ordinary
    /// gesture rather than an error.
    pub fn show(&mut self, floor: u32) {
        self.floor = floor.min(self.floors.saturating_sub(1));
    }

    /// Is a floor the one being shown?
    pub fn shows(&self, floor: u32) -> bool {
        self.floor == floor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cv_bindings::{DialBounds, DialValue};

    fn table() -> UnlockView {
        let mut t = UnlockView::new();
        t.add("u_7f3a91", "PullToAnchor", "can tether to an anchor");
        t.add("u_2c14e8", "LongPullToAnchor", "a longer tether");
        t.supersede("u_2c14e8", "u_7f3a91");
        t
    }

    #[test]
    fn the_id_column_is_read_only_and_the_view_shows_that() {
        // ⚠ A developer who believes the name is the identity will avoid renaming — the opposite of the
        // property.
        let columns: BTreeMap<&str, Editable> = UNLOCK_COLUMNS.into_iter().collect();
        assert_eq!(columns["id"], Editable::Never);
        assert_eq!(columns["name"], Editable::Text);
        assert_eq!(columns["supersedes"], Editable::PickFromTable);
    }

    #[test]
    fn renaming_edits_name_only_and_every_edge_survives() {
        let mut t = table();
        assert!(t.rename("u_7f3a91", "AnchorTether"));
        assert_eq!(t.rows()[0].name, "AnchorTether");
        assert_eq!(t.rows()[0].id, "u_7f3a91", "the id did not move");
        assert_eq!(
            t.rows()[1].supersedes,
            vec!["u_7f3a91".to_string()],
            "and the edge still points at it"
        );
        assert!(!t.rename("u_nope", "x"));
    }

    #[test]
    fn a_supersedes_picker_offers_every_other_row_and_never_itself() {
        // ⚠ Offering itself would let a developer author the smallest cycle with one click.
        let t = table();
        assert_eq!(t.supersedes_options("u_7f3a91"), vec!["u_2c14e8"]);
        assert!(!t.supersedes_options("u_2c14e8").contains(&"u_2c14e8"));
    }

    #[test]
    fn a_cycle_is_shown_in_the_table_rather_than_deferred_to_a_build() {
        // ⚠ The developer is looking at the two rows that make the loop.
        let mut t = UnlockView::new();
        t.add("a", "A", "");
        t.add("b", "B", "");
        t.supersede("a", "b");
        t.supersede("b", "a");

        let findings = t.check();
        let cycle = findings
            .iter()
            .find(|f| matches!(f, TableFinding::Cycle { .. }))
            .expect("the loop is drawn");
        assert!(cycle.to_string().contains("loop"));
        assert!(cycle.blocks());
    }

    #[test]
    fn a_healthy_table_has_nothing_to_draw() {
        assert!(table().check().is_empty());
    }

    #[test]
    fn a_dangling_supersedes_names_both_ends() {
        let mut t = UnlockView::new();
        t.add("a", "A", "");
        t.supersede("a", "ghost");
        assert_eq!(
            t.check(),
            vec![TableFinding::Dangling {
                row: "a".into(),
                missing: "ghost".into()
            }]
        );
    }

    #[test]
    fn two_rows_with_the_same_name_warn_without_blocking() {
        // ⚠ Names are labels, so it is legal — but a picker showing two identical entries is unusable.
        let mut t = UnlockView::new();
        t.add("a", "Boots", "");
        t.add("b", "Boots", "");
        let findings = t.check();
        assert_eq!(findings.len(), 1);
        assert!(!findings[0].blocks());
        assert!(findings[0].to_string().contains("unusable"));
    }

    fn dials() -> Dials {
        let mut d = Dials::new();
        d.declare(
            DialMeta::authored(
                "/Content/Items/Hookshot",
                "length",
                DialValue::Number(30.0),
                DialBounds::number(8.0, 200.0),
            )
            .documented("how far the rope reaches"),
        );
        d.declare(DialMeta::authored(
            "/Content/Items/Hookshot",
            "wear_rate",
            DialValue::Curve {
                asset: "/Content/Curves/wear.cvcurve".into(),
                row: "rate".into(),
            },
            DialBounds::default(),
        ));
        d.declare(DialMeta::authored(
            "/Content/Spines/Ascent",
            "room_count",
            DialValue::Adaptive {
                soft_min: 3.0,
                hard_max: 5.0,
            },
            DialBounds::adaptive(3.0, 5.0),
        ));
        d
    }

    #[test]
    fn the_dials_view_is_project_dials_and_holds_no_state_of_its_own() {
        // ⚠ The panel and a host cannot disagree about what a dial is set to.
        let d = dials();
        let rows = DialsView::rows(&d);
        assert_eq!(rows.len(), d.len());
        for row in &rows {
            assert!(d.get(&row.id).is_ok(), "{} came from the seam", row.id);
        }
    }

    #[test]
    fn rows_are_grouped_by_scope_then_owner() {
        // ⚠ A developer knows *where* a dial applies before they know which class declared it.
        let d = dials();
        let rows = DialsView::rows(&d);
        let keys: Vec<(String, String)> = rows
            .iter()
            .map(|r| (r.scope.clone(), r.owner.clone()))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn a_curve_valued_dial_is_a_thumbnail_rather_than_a_number() {
        // ⚠ There is no number to show — a curve has a shape, and its first key is one point of it.
        let d = dials();
        let rows = DialsView::rows(&d);
        let curve = rows.iter().find(|r| r.id.ends_with("wear_rate")).unwrap();
        assert_eq!(curve.rendering, Rendering::Thumbnail);

        let number = rows.iter().find(|r| r.id.ends_with(".length")).unwrap();
        assert_eq!(number.rendering, Rendering::Number);

        let adaptive = rows.iter().find(|r| r.id.ends_with("room_count")).unwrap();
        assert_eq!(adaptive.rendering, Rendering::Pair);
    }

    #[test]
    fn a_row_says_which_scope_supplied_its_value() {
        // ⚠ With a scoped override in play, the number in the panel is not the number a given room uses.
        let mut d = dials();
        let rows = DialsView::rows(&d);
        assert_eq!(rows[0].supplied_by, "content");
        assert!(!rows[0].overridden);

        d.set("Hookshot.length", DialValue::Number(45.0), Some("area_1"))
            .unwrap();
        let after = DialsView::rows(&d);
        let row = after.iter().find(|r| r.id == "Hookshot.length").unwrap();
        assert!(row.overridden);
        assert_eq!(row.supplied_by, "host, for area_1");
        assert_eq!(row.scope, "area_1");
    }

    #[test]
    fn search_matches_the_doc_as_well_as_the_id() {
        // ⚠ A developer who remembers "the one about rope length" is the ordinary case.
        let d = dials();
        let rows = DialsView::rows(&d);
        assert_eq!(DialsView::search(&rows, "rope").len(), 1);
        assert_eq!(DialsView::search(&rows, "hookshot").len(), 2);
        assert_eq!(
            DialsView::search(&rows, "LENGTH").len(),
            1,
            "case-insensitive"
        );
        assert!(DialsView::search(&rows, "nothing-like-this").is_empty());
    }

    #[test]
    fn the_dials_view_creates_nothing() {
        // ⚠ Creation is the DIALS section on the owner; this view turns knobs.
        let d = dials();
        let before = d.len();
        let _ = DialsView::rows(&d);
        assert_eq!(d.len(), before);
    }

    #[test]
    fn the_floor_slider_is_shared_state_and_clamps_rather_than_refusing() {
        // ⚠ A drag past the end is an ordinary gesture.
        let mut slider = FloorSlider::over(3);
        assert!(slider.shows(0));
        slider.show(2);
        assert!(slider.shows(2));
        slider.show(99);
        assert!(slider.shows(2), "clamped to the last floor");

        let empty = FloorSlider::over(0);
        assert!(empty.shows(0), "a scope with no floors still has a slider");
    }
}
