//! **Dials** — the developer's tuning channel.
//!
//! ⚠ **Always user-defined, always optional, and the core ships none.** There is no such thing as a
//! core dial. A developer authors one to tune any behaviour they intend — their own or the core's —
//! and it is the *developer's graph* that reads it and influences generation. A dial named
//! `cycle_density` is a developer's variable, not a knob the core published.
//!
//! ⚠ **A dial that reaches nothing is not a dial.** The read path is [`ResolvedDials::get`], and the
//! first real consumer is `cycle_density` driving the solver's shortcut budget. If an authored dial
//! could not reach generation, the whole category would be a table of names.
//!
//! # Identity is `<ClassName>.<DialName>`
//!
//! Both halves are authored, so a dial needs no separate id and host code names what the developer
//! named: `Hookshot.length`, `Anything.fire_affinity`. ⚠ **The class half is not optional** — a dial is
//! resolved against a *scope*, and a scope may be any kind, so the owner is never implied by where the
//! value was set.
//!
//! # Resolution: outward-in, inner scope wins
//!
//! A value set at World applies everywhere; the same dial set at one Area overrides it *there and
//! below*. ⚠ **The trace records which scope supplied the value**, because *"why is this room like
//! this?"* is unanswerable otherwise — the number alone cannot say whether it came from the project
//! default or from an override two levels up.
//!
//! # Resolve once per pass, structurally
//!
//! ⚠ Every dial reachable from a pass is resolved **up front** into an immutable table the pass reads.
//! Nothing re-reads a dial mid-pass — and that is a property of the *type*, not a rule callers are
//! asked to respect: [`ResolvedDials`] has no interior mutability and no access to the book it came
//! from, so re-reading is not something a caller could do by mistake.
//!
//! # Changing a dial is a different recipe
//!
//! ⚠ **A changed dial regenerates the world in full.** A dial is an input to the recipe, so changing
//! one changes the fingerprint. Partial regeneration is not merely hard, it is *wrong*: decisions made
//! against the old value would survive, and no seed would explain the result. Two cases are **not**
//! regeneration and stay free — a dial set before the first pass, and one written onto a scope still
//! `Projected`, which is the lookbehind channel lazy generation depends on.

use crate::axis::{AxisBook, AxisInput};
use crate::curve::{CurveBook, CurveError};
use crate::node::{Node, NodeGraph, NodeKind};
use crate::path::AssetPath;
use crate::schedule::AdaptiveRange;
use crate::Handle;
use std::collections::BTreeMap;
use std::fmt;

/// A dial's identity — `<ClassName>.<DialName>`.
///
/// ⚠ **Both halves authored.** A bare `length` would collide the moment two classes both have one, and
/// the collision would be silent because both resolve.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DialId {
    class: String,
    name: String,
}

impl DialId {
    /// From the two halves.
    pub fn new(class: &str, name: &str) -> Self {
        DialId {
            class: class.to_string(),
            name: name.to_string(),
        }
    }

    /// Parse `Class.dial`. The **last** dot separates, so a dotted class name still works.
    pub fn parse(qualified: &str) -> Option<Self> {
        let (class, name) = qualified.rsplit_once('.')?;
        if class.is_empty() || name.is_empty() {
            return None;
        }
        Some(DialId::new(class, name))
    }

    /// The owning class.
    pub fn class(&self) -> &str {
        &self.class
    }

    /// The dial's own name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for DialId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.class, self.name)
    }
}

// ⚠ **`AdaptiveRange` is [`crate::schedule::AdaptiveRange`] and is not redeclared here.** The
// scheduler already owns *"a soft minimum, a hard maximum, and a target computed from what is genuinely
// available"*, along with `repeat_tol` and `jitter` — a second one would be the same idea with a
// slightly different formula, and the two would drift the moment either was tuned.

/// What a dial holds — **all six forms**, because two would not cover what a project tunes.
#[derive(Clone, Debug, PartialEq)]
pub enum DialValue {
    /// A single number.
    Number(f64),
    /// **Hard** bounds the generator may not leave.
    Range { min: f64, max: f64 },
    /// A soft floor and a hard ceiling — see [`AdaptiveRange`].
    Adaptive(AdaptiveRange),
    /// One member of a named enum.
    Enum { enum_path: String, value: String },
    /// One row of a curve table, read at the table's own domain axis.
    Curve { table: AssetPath, row: String },
    /// A whole table, driving one named eval input.
    ///
    /// ⚠ **Drives that input for every row at once**, which is the difference from `Curve`: a `Curve`
    /// dial *is* a number that varies; a `Table` dial *moves* where every row is read.
    Table { table: AssetPath, eval: String },
}

impl DialValue {
    /// A constant.
    pub fn number(v: f64) -> Self {
        DialValue::Number(v)
    }

    /// Hard bounds.
    pub fn range(min: f64, max: f64) -> Self {
        DialValue::Range {
            min: min.min(max),
            max: min.max(max),
        }
    }

    /// A soft floor and a hard ceiling.
    pub fn adaptive(range: AdaptiveRange) -> Self {
        DialValue::Adaptive(range)
    }

    /// An enum member.
    pub fn enum_value(enum_path: &str, value: &str) -> Self {
        DialValue::Enum {
            enum_path: enum_path.to_string(),
            value: value.to_string(),
        }
    }

    /// One row of a table.
    pub fn curve(table: AssetPath, row: &str) -> Self {
        DialValue::Curve {
            table,
            row: row.to_string(),
        }
    }

    /// A whole table, driving one eval input.
    pub fn table(table: AssetPath, eval: &str) -> Self {
        DialValue::Table {
            table,
            eval: eval.to_string(),
        }
    }

    /// The form's name, as the format writes it.
    pub fn kind(&self) -> &'static str {
        match self {
            DialValue::Number(_) => "Number",
            DialValue::Range { .. } => "Range",
            DialValue::Adaptive(_) => "Adaptive",
            DialValue::Enum { .. } => "Enum",
            DialValue::Curve { .. } => "Curve",
            DialValue::Table { .. } => "Table",
        }
    }

    /// Does this form resolve to a number at all?
    ///
    /// ⚠ `Enum` and `Table` do not, and that is not a gap: an enum names a choice and a table dial
    /// *moves an axis* rather than being a value. A caller that wanted a number from either has asked
    /// the wrong question, and [`ResolvedDials::number`] says so rather than inventing one.
    pub fn is_numeric(&self) -> bool {
        !matches!(self, DialValue::Enum { .. } | DialValue::Table { .. })
    }
}

/// What can go wrong resolving a dial.
#[derive(Clone, Debug, PartialEq)]
pub enum DialError {
    /// A curve-valued dial names a table nothing loaded.
    NoSuchTable { dial: DialId, table: String },
    /// The table could not be read.
    Curve { dial: DialId, source: CurveError },
    /// The table's domain names an axis nothing provides.
    UnboundDomain { dial: DialId, domain: String },
}

impl fmt::Display for DialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialError::NoSuchTable { dial, table } => {
                write!(f, "{dial} reads {table}, which is not loaded")
            }
            DialError::Curve { dial, source } => write!(f, "{dial}: {source}"),
            DialError::UnboundDomain { dial, domain } => write!(
                f,
                "{dial} reads a table over {domain:?}, and no ProgressionAxis carries that name"
            ),
        }
    }
}

impl std::error::Error for DialError {}

/// One authored dial value, and the scope it was set at.
#[derive(Clone, Debug, PartialEq)]
struct Setting {
    value: DialValue,
    scope: Handle<Node>,
}

/// **Every authored dial value, per scope.** What a project writes into.
///
/// ⚠ **Not what a pass reads.** A pass reads a [`ResolvedDials`], which this produces once and then has
/// no further part in — see the module note on resolving once.
#[derive(Clone, Debug, Default)]
pub struct DialBook {
    settings: BTreeMap<DialId, Vec<Setting>>,
}

impl DialBook {
    /// An empty book.
    pub fn new() -> Self {
        DialBook::default()
    }

    /// Set a dial at a scope. A second set at the same scope replaces the first.
    pub fn set(&mut self, id: DialId, scope: Handle<Node>, value: DialValue) {
        let entries = self.settings.entry(id).or_default();
        match entries.iter_mut().find(|s| s.scope == scope) {
            Some(existing) => existing.value = value,
            None => entries.push(Setting { value, scope }),
        }
    }

    /// Set a dial at a scope, chaining.
    pub fn with(mut self, id: DialId, scope: Handle<Node>, value: DialValue) -> Self {
        self.set(id, scope, value);
        self
    }

    /// How many distinct dials are authored.
    pub fn len(&self) -> usize {
        self.settings.len()
    }

    /// Is the book empty?
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }

    /// Every authored dial id, in order.
    pub fn ids(&self) -> impl Iterator<Item = &DialId> {
        self.settings.keys()
    }

    /// Every setting for a dial, as stable text — what the fingerprint folds in.
    ///
    /// ⚠ **Sorted by scope index rather than insertion order**, so two books that authored the same
    /// values in a different order fingerprint the same. Authoring order is not part of the recipe.
    pub fn describe(&self, id: &DialId) -> String {
        let Some(entries) = self.settings.get(id) else {
            return String::new();
        };
        let mut parts: Vec<String> = entries
            .iter()
            .map(|s| format!("{}={:?}", s.scope.index(), s.value))
            .collect();
        parts.sort();
        parts.join(";")
    }

    /// **Which setting applies at this scope** — the innermost one on the path to the root.
    ///
    /// ⚠ **Inner wins**, walking outward from the scope. A value set at World applies everywhere; the
    /// same dial set at one Area overrides it there and below, and nowhere else.
    fn applicable(&self, id: &DialId, graph: &NodeGraph, scope: Handle<Node>) -> Option<&Setting> {
        let entries = self.settings.get(id)?;
        let mut cursor = Some(scope);
        for _ in 0..=NodeKind::ALL.len() {
            let h = cursor?;
            if let Some(s) = entries.iter().find(|s| s.scope == h) {
                return Some(s);
            }
            cursor = graph.get(h).and_then(Node::parent);
        }
        None
    }
}

/// One resolved dial: its value, and **where the value came from**.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    /// The value that applies.
    pub value: DialValue,
    /// The scope that supplied it.
    ///
    /// ⚠ **The trace's answer to *"why is this room like this?"***. The number alone cannot say
    /// whether it came from the project default or an override two levels up, and a developer
    /// chasing an unexpected world needs exactly that.
    pub from_scope: Handle<Node>,
    /// Which kind of scope that was, for a readable trace.
    pub from_kind: NodeKind,
    /// For a `Curve` dial, the number it read to — resolved once, here.
    pub sampled: Option<f64>,
}

/// **The immutable table a pass reads.**
///
/// ⚠ **Resolving once is structural, not a convention.** This owns no reference to the [`DialBook`],
/// no axis book and no interior mutability, so re-reading a dial mid-pass is not something a caller
/// could do by mistake — there is nothing here to re-read *from*.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedDials {
    /// Keyed by dial and scope, because one dial resolves differently per scope.
    values: BTreeMap<(DialId, u32), Resolved>,
    /// How many times [`DialBook::applicable`] ran while building this — the property a
    /// resolve-once test asserts on.
    reads: usize,
}

impl ResolvedDials {
    /// **Resolve every authored dial, for every scope, once.**
    ///
    /// Curve-valued dials are sampled here, which is the whole point: the pass gets a number, and the
    /// axis is never consulted again while decisions are being made against it.
    pub fn resolve(
        book: &DialBook,
        graph: &NodeGraph,
        curves: &CurveBook,
        axes: &AxisBook,
        occupant_unlocks: usize,
    ) -> Result<Self, DialError> {
        let mut out = ResolvedDials::default();
        for (index, (scope, node)) in graph.iter().enumerate() {
            let _ = index;
            for id in book.ids() {
                let Some(setting) = book.applicable(id, graph, scope) else {
                    continue;
                };
                out.reads += 1;

                let sampled = match &setting.value {
                    DialValue::Curve { table, row } => {
                        let t = curves.get(table).ok_or_else(|| DialError::NoSuchTable {
                            dial: id.clone(),
                            table: table.to_string(),
                        })?;
                        let input = AxisInput {
                            graph,
                            scope,
                            unlocks_held: occupant_unlocks,
                            sphere: None,
                            sphere_count: 0,
                            unlock_total: 0,
                        };
                        let x = axes.value(t.domain(), &input).ok_or_else(|| {
                            DialError::UnboundDomain {
                                dial: id.clone(),
                                domain: t.domain().to_string(),
                            }
                        })?;
                        Some(t.sample(row, x).map_err(|source| DialError::Curve {
                            dial: id.clone(),
                            source,
                        })?)
                    }
                    _ => None,
                };

                out.values.insert(
                    (id.clone(), scope.index()),
                    Resolved {
                        value: setting.value.clone(),
                        from_scope: setting.scope,
                        from_kind: graph
                            .get(setting.scope)
                            .map(Node::kind)
                            .unwrap_or(NodeKind::World),
                        sampled,
                    },
                );
                let _ = node;
            }
        }
        Ok(out)
    }

    /// What applies at this scope, with its provenance.
    pub fn get(&self, id: &DialId, scope: Handle<Node>) -> Option<&Resolved> {
        self.values.get(&(id.clone(), scope.index()))
    }

    /// The number a dial resolves to at this scope.
    ///
    /// ⚠ `None` for `Enum` and `Table`, which do not resolve to numbers — an enum names a choice and a
    /// table dial *moves an axis*. Inventing a number for either would let a caller's wrong question
    /// look like a right answer.
    pub fn number(&self, id: &DialId, scope: Handle<Node>) -> Option<f64> {
        let r = self.get(id, scope)?;
        match &r.value {
            DialValue::Number(v) => Some(*v),
            DialValue::Range { min, max } => Some((min + max) * 0.5),
            DialValue::Adaptive(a) => Some(a.hard_max as f64),
            DialValue::Curve { .. } => r.sampled,
            DialValue::Enum { .. } | DialValue::Table { .. } => None,
        }
    }

    /// A number, clamped into a `Range` dial's hard bounds.
    ///
    /// ⚠ **Hard bounds the generator may not leave** — so this is the call a consumer makes when it has
    /// its own candidate and needs it made legal, rather than reading the midpoint.
    pub fn clamp(&self, id: &DialId, scope: Handle<Node>, candidate: f64) -> f64 {
        match self.get(id, scope).map(|r| &r.value) {
            Some(DialValue::Range { min, max }) => candidate.clamp(*min, *max),
            Some(DialValue::Adaptive(a)) => candidate.clamp(0.0, a.hard_max as f64),
            _ => candidate,
        }
    }

    /// How many dial lookups building this table performed.
    ///
    /// ⚠ **The number the resolve-once property test asserts on**: it must not grow with how many
    /// graphs reference a dial, only with how many dials and scopes exist.
    pub fn reads(&self) -> usize {
        self.reads
    }

    /// How many (dial, scope) pairs resolved.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Did anything resolve?
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// A one-line trace of where a value came from.
    pub fn explain(&self, id: &DialId, scope: Handle<Node>) -> Option<String> {
        let r = self.get(id, scope)?;
        Some(match r.sampled {
            Some(v) => format!(
                "{id} = {v} (a {} set at {:?}, sampled)",
                r.value.kind(),
                r.from_kind
            ),
            None => format!("{id} = {:?} set at {:?}", r.value, r.from_kind),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axis::AxisBook;
    use crate::curve::{CurveBook, CurveTable, Row};
    use crate::schedule::Curve;

    fn asset(p: &str) -> AssetPath {
        AssetPath::new(p).unwrap()
    }

    /// World ▸ 2 Reaches ▸ 1 Area each ▸ 1 Space each.
    fn world() -> (NodeGraph, Vec<Handle<Node>>) {
        let mut g = NodeGraph::new(1.0, 11);
        let root = g.root();
        let mut handles = vec![root];
        for i in 0..2 {
            let r = g.add_child(root, format!("reach_{i}")).unwrap();
            let a = g.add_child(r, format!("area_{i}")).unwrap();
            let s = g.add_child(a, format!("space_{i}")).unwrap();
            handles.extend([r, a, s]);
        }
        (g, handles)
    }

    fn curves() -> CurveBook {
        CurveBook::new().with(
            CurveTable::new(asset("/Content/Curves/p.cvcurve"), "depth").row(
                "complexity",
                Row::linear(Curve::from_points([(0.0, 0.2), (1.0, 1.0)])),
            ),
        )
    }

    // --- identity ------------------------------------------------------------------------------

    #[test]
    fn identity_is_both_halves_because_a_bare_name_collides() {
        // ⚠ Two classes both having a `length` is ordinary; a bare name makes them one dial, and the
        // collision is silent because both resolve.
        let a = DialId::new("Hookshot", "length");
        let b = DialId::new("Rope", "length");
        assert_ne!(a, b);
        assert_eq!(a.to_string(), "Hookshot.length");
        assert_eq!(DialId::parse("Hookshot.length"), Some(a));
        assert_eq!(DialId::parse("length"), None, "a bare name is not an id");
    }

    #[test]
    fn parsing_splits_on_the_last_dot_so_a_dotted_class_still_works() {
        let id = DialId::parse("Content.Items.Hookshot.length").unwrap();
        assert_eq!(id.class(), "Content.Items.Hookshot");
        assert_eq!(id.name(), "length");
    }

    // --- the six forms ---------------------------------------------------------------------------

    #[test]
    fn all_six_forms_exist_because_two_would_not_cover_what_a_project_tunes() {
        let forms = [
            DialValue::number(3.0),
            DialValue::range(1.0, 9.0),
            DialValue::adaptive(AdaptiveRange::new(4, 12)),
            DialValue::enum_value("/Core/InstanceScope", "AREA"),
            DialValue::curve(asset("/Content/Curves/p.cvcurve"), "complexity"),
            DialValue::table(asset("/Content/Curves/p.cvcurve"), "depth"),
        ];
        let kinds: Vec<&str> = forms.iter().map(DialValue::kind).collect();
        assert_eq!(
            kinds,
            vec!["Number", "Range", "Adaptive", "Enum", "Curve", "Table"]
        );
    }

    #[test]
    fn an_adaptive_dial_carries_the_schedulers_range_rather_than_a_second_one() {
        // ⚠ *"A soft minimum, a hard maximum, and a target computed from what is genuinely
        // available"* already exists in the scheduler, with `repeat_tol` and `jitter`. A second
        // AdaptiveRange would be the same idea with a slightly different formula, and the two would
        // drift the moment either was tuned.
        let d = DialValue::adaptive(AdaptiveRange::new(4, 12));
        let DialValue::Adaptive(a) = d else {
            panic!("an adaptive dial")
        };
        assert_eq!(a.soft_min, 4);
        assert_eq!(a.hard_max, 12);
    }

    #[test]
    fn reversed_bounds_are_sorted_rather_than_rejected() {
        assert_eq!(DialValue::range(9.0, 1.0), DialValue::range(1.0, 9.0));
    }

    // --- resolution walks the ladder ---------------------------------------------------------------

    #[test]
    fn a_world_value_applies_everywhere_and_an_inner_one_overrides_it_there() {
        // ⚠ The resolution rule, stated as a test: outward-in, inner scope wins, and the override
        // reaches *there and below* rather than everywhere.
        let (g, h) = world();
        let (root, area_0, space_0, space_1) = (h[0], h[2], h[3], h[6]);
        let id = DialId::new("Area", "cycle_density");

        let book = DialBook::new()
            .with(id.clone(), root, DialValue::number(0.2))
            .with(id.clone(), area_0, DialValue::number(0.9));

        let r =
            ResolvedDials::resolve(&book, &g, &curves(), &AxisBook::with_builtins(), 0).unwrap();

        assert_eq!(r.number(&id, space_0), Some(0.9), "under the override");
        assert_eq!(r.number(&id, space_1), Some(0.2), "the other branch");
        assert_eq!(r.number(&id, root), Some(0.2));
    }

    #[test]
    fn the_trace_records_which_scope_supplied_the_value() {
        // ⚠ *"Why is this room like this?"* is unanswerable from the number alone — it cannot say
        // whether the value came from the project default or an override two levels up.
        let (g, h) = world();
        let (root, area_0, space_0, space_1) = (h[0], h[2], h[3], h[6]);
        let id = DialId::new("Area", "cycle_density");
        let book = DialBook::new()
            .with(id.clone(), root, DialValue::number(0.2))
            .with(id.clone(), area_0, DialValue::number(0.9));
        let r =
            ResolvedDials::resolve(&book, &g, &curves(), &AxisBook::with_builtins(), 0).unwrap();

        assert_eq!(r.get(&id, space_0).unwrap().from_scope, area_0);
        assert_eq!(r.get(&id, space_0).unwrap().from_kind, NodeKind::Area);
        assert_eq!(r.get(&id, space_1).unwrap().from_scope, root);
        assert_eq!(r.get(&id, space_1).unwrap().from_kind, NodeKind::World);
        assert!(r.explain(&id, space_0).unwrap().contains("Area"));
    }

    #[test]
    fn an_unset_dial_resolves_to_nothing_rather_than_a_default() {
        // ⚠ The core ships no dials, so it has no default to fall back to. A number here would be the
        // core inventing a tuning value nobody authored.
        let (g, h) = world();
        let id = DialId::new("Area", "never_set");
        let r = ResolvedDials::resolve(
            &DialBook::new(),
            &g,
            &curves(),
            &AxisBook::with_builtins(),
            0,
        )
        .unwrap();
        assert_eq!(r.get(&id, h[3]), None);
        assert_eq!(r.number(&id, h[3]), None);
    }

    // --- curve-valued dials -------------------------------------------------------------------

    #[test]
    fn linear_at_first_opening_up_the_deeper_you_go() {
        // ⚠ **The milestone's shape.** One authored curve, read at each scope's own depth — the value
        // varies across the world without anyone writing a value per scope.
        let (g, h) = world();
        let (root, space_0, space_1) = (h[0], h[3], h[6]);
        let id = DialId::new("Area", "complexity");

        let book = DialBook::new().with(
            id.clone(),
            root,
            DialValue::curve(asset("/Content/Curves/p.cvcurve"), "complexity"),
        );
        let r =
            ResolvedDials::resolve(&book, &g, &curves(), &AxisBook::with_builtins(), 0).unwrap();

        assert_eq!(r.number(&id, space_0), Some(0.2), "the first Reach");
        assert_eq!(r.number(&id, space_1), Some(1.0), "the last");
    }

    #[test]
    fn a_curve_dial_naming_a_missing_table_fails_at_resolve_rather_than_at_use() {
        let (g, h) = world();
        let id = DialId::new("Area", "complexity");
        let book = DialBook::new().with(
            id,
            h[0],
            DialValue::curve(asset("/Content/Curves/absent.cvcurve"), "complexity"),
        );
        assert!(matches!(
            ResolvedDials::resolve(&book, &g, &curves(), &AxisBook::with_builtins(), 0),
            Err(DialError::NoSuchTable { .. })
        ));
    }

    #[test]
    fn a_curve_dial_over_an_unbound_axis_fails_at_resolve() {
        // ⚠ The axis lint, reached through a dial: a table read over a domain nothing provides is a
        // load-time diagnostic, never a zero.
        let (g, h) = world();
        let id = DialId::new("Area", "complexity");
        let curves = CurveBook::new().with(
            CurveTable::new(asset("/Content/Curves/p.cvcurve"), "boss_count")
                .row("complexity", Row::linear(Curve::constant(1.0))),
        );
        let book = DialBook::new().with(
            id,
            h[0],
            DialValue::curve(asset("/Content/Curves/p.cvcurve"), "complexity"),
        );
        assert!(matches!(
            ResolvedDials::resolve(&book, &g, &curves, &AxisBook::with_builtins(), 0),
            Err(DialError::UnboundDomain { .. })
        ));
    }

    #[test]
    fn a_curve_dial_naming_a_missing_row_fails_at_resolve() {
        let (g, h) = world();
        let id = DialId::new("Area", "complexity");
        let book = DialBook::new().with(
            id,
            h[0],
            DialValue::curve(asset("/Content/Curves/p.cvcurve"), "complexty"),
        );
        assert!(matches!(
            ResolvedDials::resolve(&book, &g, &curves(), &AxisBook::with_builtins(), 0),
            Err(DialError::Curve { .. })
        ));
    }

    // --- what does and does not resolve to a number -------------------------------------------

    #[test]
    fn an_enum_and_a_table_dial_do_not_pretend_to_be_numbers() {
        // ⚠ An enum names a choice; a table dial *moves an axis*. Inventing a number for either would
        // let a caller's wrong question look like a right answer.
        let (g, h) = world();
        let e = DialId::new("Area", "scope");
        let t = DialId::new("Area", "drive");
        let book = DialBook::new()
            .with(
                e.clone(),
                h[0],
                DialValue::enum_value("/Core/InstanceScope", "AREA"),
            )
            .with(
                t.clone(),
                h[0],
                DialValue::table(asset("/Content/Curves/p.cvcurve"), "depth"),
            );
        let r =
            ResolvedDials::resolve(&book, &g, &curves(), &AxisBook::with_builtins(), 0).unwrap();

        assert_eq!(r.number(&e, h[3]), None);
        assert_eq!(r.number(&t, h[3]), None);
        assert!(r.get(&e, h[3]).is_some(), "but both still resolved");
        assert!(!DialValue::enum_value("/Core/X", "Y").is_numeric());
    }

    #[test]
    fn a_hard_range_clamps_a_candidate_rather_than_replacing_it() {
        // ⚠ Hard bounds are what the generator *may not leave*, so the useful call is one that makes
        // a candidate legal — reading the midpoint would throw the generator's own reasoning away.
        let (g, h) = world();
        let id = DialId::new("Area", "rooms");
        let book = DialBook::new().with(id.clone(), h[0], DialValue::range(4.0, 9.0));
        let r =
            ResolvedDials::resolve(&book, &g, &curves(), &AxisBook::with_builtins(), 0).unwrap();

        assert_eq!(r.clamp(&id, h[3], 7.0), 7.0, "already legal");
        assert_eq!(r.clamp(&id, h[3], 99.0), 9.0);
        assert_eq!(r.clamp(&id, h[3], 0.0), 4.0);
        assert_eq!(r.number(&id, h[3]), Some(6.5), "the midpoint, if asked");
    }

    #[test]
    fn clamping_a_dial_that_is_not_a_range_leaves_the_candidate_alone() {
        let (g, h) = world();
        let id = DialId::new("Area", "x");
        let book = DialBook::new().with(id.clone(), h[0], DialValue::number(3.0));
        let r =
            ResolvedDials::resolve(&book, &g, &curves(), &AxisBook::with_builtins(), 0).unwrap();
        assert_eq!(r.clamp(&id, h[3], 99.0), 99.0);
        assert_eq!(r.clamp(&DialId::new("Area", "absent"), h[3], 99.0), 99.0);
    }

    // --- resolve once -----------------------------------------------------------------------------

    #[test]
    fn a_pass_reads_each_dial_once_per_scope_however_many_graphs_reference_it() {
        // ⚠ **Structural, not conventional.** `ResolvedDials` holds no reference to the book, no axis
        // book and no interior mutability — re-reading is not something a caller could do by mistake,
        // because there is nothing here to re-read *from*.
        let (g, h) = world();
        let id = DialId::new("Area", "x");
        let book = DialBook::new().with(id.clone(), h[0], DialValue::number(1.0));
        let r =
            ResolvedDials::resolve(&book, &g, &curves(), &AxisBook::with_builtins(), 0).unwrap();

        let scopes = g.iter().count();
        assert_eq!(r.reads(), scopes, "one lookup per scope, and no more");
        assert_eq!(r.len(), scopes);

        // Reading it a thousand times costs nothing further — the count is fixed at resolve time.
        for _ in 0..1_000 {
            assert_eq!(r.number(&id, h[3]), Some(1.0));
        }
        assert_eq!(r.reads(), scopes);
    }

    #[test]
    fn setting_one_dial_twice_at_one_scope_replaces_rather_than_stacking() {
        let (g, h) = world();
        let id = DialId::new("Area", "x");
        let mut book = DialBook::new();
        book.set(id.clone(), h[0], DialValue::number(1.0));
        book.set(id.clone(), h[0], DialValue::number(2.0));
        let r =
            ResolvedDials::resolve(&book, &g, &curves(), &AxisBook::with_builtins(), 0).unwrap();
        assert_eq!(r.number(&id, h[3]), Some(2.0));
        assert_eq!(book.len(), 1);
    }
}
