//! **`ProgressionAxis`** — the `x` a curve row is read at.
//!
//! A curve says *what the value is at a given point in the run*. An axis says *what point in the run
//! we are at*. Splitting them is what lets one authored curve be read over depth in one project and
//! over boss count in another, without the curve knowing.
//!
//! # Binding is by name
//!
//! ⚠ A table declares `domain = "boss_count"` and whichever axis **carries that name** supplies the
//! number. Not passed at each call site, and that is the point: a table naming a domain no axis
//! provides becomes a **load-time diagnostic** rather than a caller who forgot an argument. The check
//! is [`AxisBook::check`], and the editor's axis lint is the same question asked in a UI.
//!
//! # Four built-ins, and the fifth is a developer's
//!
//! `depth` · `space_count` · `unlock_count` · `sphere` cover what the core can compute from the graph
//! alone. Anything else is a developer's axis with one graph — *"complexity gains weight each time a
//! boss is placed"* is a `ProgressionAxis` named `boss_count`, **and there is no other way to say it**.
//! That is why the trait is public and the built-ins are not a closed set.
//!
//! ⚠ **An axis reports position, never preference.** It answers *"how far in are we"*; what that should
//! do to generation is the curve's business. An axis that returned a weight would be a dial wearing an
//! axis's name, and the two would be impossible to retune independently.

use crate::node::{Node, NodeGraph, NodeKind};
use crate::Handle;
use cv_determinism::math;
use std::collections::BTreeMap;
use std::fmt;

/// What an axis is asked about: one scope, in one world.
///
/// ⚠ **Deliberately not `Context`.** An axis runs during dial resolution, before most of a pass exists;
/// handing it the full context would let one read a dial, and a dial that read an axis that read a dial
/// is a cycle nothing could order.
#[derive(Clone, Copy, Debug)]
pub struct AxisInput<'a> {
    /// The world graph so far.
    pub graph: &'a NodeGraph,
    /// The scope being asked about.
    pub scope: Handle<Node>,
    /// How many unlocks the occupant is assumed to hold at this point.
    pub unlocks_held: usize,
    /// Which accessibility sphere this scope falls in, if the ladder has run.
    pub sphere: Option<usize>,
    /// How many spheres there are, for normalising.
    pub sphere_count: usize,
    /// How many unlocks the project declares, for normalising.
    pub unlock_total: usize,
}

/// Supplies the `x` a curve row is read at.
///
/// ⚠ **One hook, and it returns a bare number.** Anything richer would let an axis smuggle in a
/// decision, and the whole split exists so that *where we are* and *what that means* stay separable.
pub trait ProgressionAxis: fmt::Debug {
    /// What a `CurveTable`'s `domain` matches against.
    fn name(&self) -> &str;

    /// The `x`.
    ///
    /// ⚠ **Not clamped by the core.** An axis that genuinely counts — bosses placed, rooms visited —
    /// has no upper bound the core knows, and clamping it to `0..1` here would silently flatten every
    /// counted axis. A row is clamped to its own keys instead, which is where the author drew them.
    fn value(&self, input: &AxisInput<'_>) -> f64;
}

/// How deep into the world this scope sits — Reach index ÷ total, in `0..1`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Depth;

impl ProgressionAxis for Depth {
    fn name(&self) -> &str {
        "depth"
    }
    fn value(&self, input: &AxisInput<'_>) -> f64 {
        let reaches: Vec<Handle<Node>> = input
            .graph
            .iter()
            .filter(|(_, n)| n.kind() == NodeKind::Reach)
            .map(|(h, _)| h)
            .collect();
        if reaches.len() <= 1 {
            // ⚠ A one-Reach world is at the *start* of its progression, not the end. Returning 1.0
            // would make every curve read at its far key before anything had been built.
            return 0.0;
        }
        let mine = ancestor_of_kind(input.graph, input.scope, NodeKind::Reach);
        match mine.and_then(|m| reaches.iter().position(|r| *r == m)) {
            Some(i) => i as f64 / (reaches.len() - 1) as f64,
            None => 0.0,
        }
    }
}

/// How many Spaces exist so far.
///
/// ⚠ **A count, not a fraction.** *"Weight rises for the first twelve rooms and then flattens"* is a
/// curve keyed at 12, and normalising here would make that key meaningless.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpaceCount;

impl ProgressionAxis for SpaceCount {
    fn name(&self) -> &str {
        "space_count"
    }
    fn value(&self, input: &AxisInput<'_>) -> f64 {
        input
            .graph
            .iter()
            .filter(|(_, n)| n.kind() == NodeKind::Space)
            .count() as f64
    }
}

/// How many unlocks the occupant is assumed to hold here.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnlockCount;

impl ProgressionAxis for UnlockCount {
    fn name(&self) -> &str {
        "unlock_count"
    }
    fn value(&self, input: &AxisInput<'_>) -> f64 {
        input.unlocks_held as f64
    }
}

/// Which accessibility sphere this scope falls in, normalised to `0..1`.
///
/// ⚠ **Answers `0.0` while the ladder has not run**, which is the honest reading: nothing is known
/// about spheres yet, and the start of progression is the safe place to be wrong.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sphere;

impl ProgressionAxis for Sphere {
    fn name(&self) -> &str {
        "sphere"
    }
    fn value(&self, input: &AxisInput<'_>) -> f64 {
        match (input.sphere, input.sphere_count) {
            (Some(s), n) if n > 1 => math::saturate(s as f64 / (n - 1) as f64),
            _ => 0.0,
        }
    }
}

/// The nearest enclosing scope of a given kind, self included.
fn ancestor_of_kind(
    graph: &NodeGraph,
    scope: Handle<Node>,
    kind: NodeKind,
) -> Option<Handle<Node>> {
    let mut cursor = Some(scope);
    // Bounded by the ladder's depth; a malformed graph truncates rather than hanging.
    for _ in 0..=NodeKind::ALL.len() {
        let h = cursor?;
        let node = graph.get(h)?;
        if node.kind() == kind {
            return Some(h);
        }
        cursor = node.parent();
    }
    None
}

/// What can go wrong binding axes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AxisError {
    /// Two axes carry one name.
    Duplicate { name: String },
    /// A table reads over a domain no axis provides.
    Unbound { domain: String, table: String },
}

impl fmt::Display for AxisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AxisError::Duplicate { name } => write!(
                f,
                "two axes named {name:?} — a domain must resolve to exactly one"
            ),
            AxisError::Unbound { domain, table } => write!(
                f,
                "{table} is read over {domain:?}, and no ProgressionAxis carries that name"
            ),
        }
    }
}

impl std::error::Error for AxisError {}

/// Every axis a project provides, by name.
///
/// ⚠ **A name resolves to exactly one axis**, so a duplicate is refused rather than shadowed. Two axes
/// named `depth` would make *"read over depth"* mean whichever loaded last, and nothing in the table
/// would say which.
#[derive(Default)]
pub struct AxisBook {
    axes: BTreeMap<String, Box<dyn ProgressionAxis>>,
}

impl fmt::Debug for AxisBook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AxisBook")
            .field("names", &self.names().collect::<Vec<_>>())
            .finish()
    }
}

impl AxisBook {
    /// An empty book.
    pub fn new() -> Self {
        AxisBook::default()
    }

    /// The four built-ins.
    ///
    /// ⚠ **Four, not a closed set.** These are what the core can compute from the graph alone;
    /// anything a project actually wants — *"each time a boss is placed"* — is its own axis, and there
    /// is no other way to say it.
    pub fn with_builtins() -> Self {
        let mut b = AxisBook::new();
        b.add(Box::new(Depth))
            .expect("no duplicates among builtins");
        b.add(Box::new(SpaceCount)).expect("no duplicates");
        b.add(Box::new(UnlockCount)).expect("no duplicates");
        b.add(Box::new(Sphere)).expect("no duplicates");
        b
    }

    /// Register an axis.
    pub fn add(&mut self, axis: Box<dyn ProgressionAxis>) -> Result<(), AxisError> {
        let name = axis.name().to_string();
        if self.axes.contains_key(&name) {
            return Err(AxisError::Duplicate { name });
        }
        self.axes.insert(name, axis);
        Ok(())
    }

    /// Is a name bound?
    pub fn contains(&self, name: &str) -> bool {
        self.axes.contains_key(name)
    }

    /// Every bound name, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.axes.keys().map(String::as_str)
    }

    /// How many axes are bound.
    pub fn len(&self) -> usize {
        self.axes.len()
    }

    /// Is the book empty?
    pub fn is_empty(&self) -> bool {
        self.axes.is_empty()
    }

    /// Read one axis.
    ///
    /// ⚠ `None` for an unbound name rather than `0.0`: zero is a legal axis value, so substituting it
    /// would make a missing axis read as *"at the very start"* — every curve pinned to its first key,
    /// world-wide, with nothing saying why.
    pub fn value(&self, name: &str, input: &AxisInput<'_>) -> Option<f64> {
        self.axes.get(name).map(|a| a.value(input))
    }

    /// **The axis lint**: every domain a loaded table reads over must be bound.
    pub fn check(&self, curves: &crate::curve::CurveBook) -> Result<(), AxisError> {
        for table in curves.iter() {
            if !self.contains(table.domain()) {
                return Err(AxisError::Unbound {
                    domain: table.domain().to_string(),
                    table: table.path().to_string(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{CurveBook, CurveTable, Row};
    use crate::node::NodeState;
    use crate::path::AssetPath;
    use crate::schedule::Curve;

    /// A world: three Reaches, each with an Area holding two Spaces.
    fn world() -> (NodeGraph, Vec<Handle<Node>>) {
        let mut g = NodeGraph::new(1.0, 7);
        let root = g.root();
        let mut reaches = Vec::new();
        for i in 0..3 {
            let r = g.add_child(root, format!("reach_{i}")).unwrap();
            let a = g.add_child(r, format!("area_{i}")).unwrap();
            g.add_child(a, format!("space_{i}a")).unwrap();
            g.add_child(a, format!("space_{i}b")).unwrap();
            reaches.push(r);
        }
        (g, reaches)
    }

    fn input<'a>(graph: &'a NodeGraph, scope: Handle<Node>) -> AxisInput<'a> {
        AxisInput {
            graph,
            scope,
            unlocks_held: 0,
            sphere: None,
            sphere_count: 0,
            unlock_total: 0,
        }
    }

    // --- the built-ins ----------------------------------------------------------------------------

    #[test]
    fn depth_runs_from_the_first_reach_to_the_last() {
        let (g, reaches) = world();
        assert_eq!(Depth.value(&input(&g, reaches[0])), 0.0);
        assert_eq!(Depth.value(&input(&g, reaches[1])), 0.5);
        assert_eq!(Depth.value(&input(&g, reaches[2])), 1.0);
    }

    #[test]
    fn depth_of_an_inner_scope_is_the_depth_of_its_reach() {
        // A Space does not have its own depth; it inherits the one the Reach it sits in has.
        let (g, reaches) = world();
        let space = g
            .iter()
            .find(|(_, n)| n.kind() == NodeKind::Space)
            .map(|(h, _)| h)
            .unwrap();
        let mine = ancestor_of_kind(&g, space, NodeKind::Reach).unwrap();
        assert_eq!(mine, reaches[0]);
        assert_eq!(Depth.value(&input(&g, space)), 0.0);
    }

    #[test]
    fn a_one_reach_world_is_at_the_start_and_not_the_end() {
        // ⚠ Returning 1.0 would read every curve at its far key before anything had been built —
        // the optimistic direction, and wrong from the first pass.
        let mut g = NodeGraph::new(1.0, 7);
        let root = g.root();
        let r = g.add_child(root, "only").unwrap();
        assert_eq!(Depth.value(&input(&g, r)), 0.0);
    }

    #[test]
    fn space_count_is_a_count_and_not_a_fraction() {
        // ⚠ *"Weight rises for the first twelve rooms then flattens"* is a curve keyed at 12.
        // Normalising here would make that key mean nothing.
        let (g, reaches) = world();
        assert_eq!(SpaceCount.value(&input(&g, reaches[0])), 6.0);
    }

    #[test]
    fn unlock_count_reports_what_the_occupant_is_assumed_to_hold() {
        let (g, reaches) = world();
        let mut i = input(&g, reaches[0]);
        i.unlocks_held = 4;
        assert_eq!(UnlockCount.value(&i), 4.0);
    }

    #[test]
    fn sphere_answers_zero_while_the_ladder_has_not_run() {
        // ⚠ The honest reading: nothing is known about spheres yet, and the start of progression is
        // the safe place to be wrong.
        let (g, reaches) = world();
        assert_eq!(Sphere.value(&input(&g, reaches[0])), 0.0);

        let mut i = input(&g, reaches[0]);
        i.sphere = Some(2);
        i.sphere_count = 5;
        assert_eq!(Sphere.value(&i), 0.5);
    }

    // --- binding by name --------------------------------------------------------------------------

    #[test]
    fn a_table_is_read_over_whichever_axis_carries_its_domain_name() {
        // ⚠ The binding mechanism, end to end: the table says "depth", the book supplies it, and
        // neither had to name the other at the call site.
        let (g, reaches) = world();
        let book = AxisBook::with_builtins();
        let table = CurveTable::new(
            AssetPath::new("/Content/Curves/p.cvcurve").unwrap(),
            "depth",
        )
        .row(
            "complexity",
            Row::linear(Curve::from_points([(0.0, 0.2), (1.0, 1.0)])),
        );

        let x = book.value(table.domain(), &input(&g, reaches[1])).unwrap();
        assert_eq!(x, 0.5);
        // ⚠ A tolerance, not equality: the value is *interpolated*, and `0.2 + 0.5 * 0.8` is
        // `0.6000000000000001`. Asserting exact equality on a derived float is the habit the binding
        // contract exists to break.
        let y = table.sample("complexity", x).unwrap();
        assert!(math::abs(y - 0.6) < 1e-9, "expected roughly 0.6, got {y}");
    }

    #[test]
    fn an_unbound_domain_is_a_load_time_diagnostic_and_not_a_zero() {
        // ⚠ Zero is a legal axis value, so substituting it would pin every curve to its first key,
        // world-wide, with nothing saying why.
        let curves = CurveBook::new().with(CurveTable::new(
            AssetPath::new("/Content/Curves/b.cvcurve").unwrap(),
            "boss_count",
        ));
        let book = AxisBook::with_builtins();
        assert!(matches!(
            book.check(&curves),
            Err(AxisError::Unbound { .. })
        ));

        let (g, reaches) = world();
        assert_eq!(book.value("boss_count", &input(&g, reaches[0])), None);
    }

    #[test]
    fn a_developers_axis_is_the_only_way_to_say_what_the_core_cannot_compute() {
        // ⚠ *"Complexity gains weight each time a boss is placed"* — the core cannot derive this, and
        // the trait being public is what makes it expressible at all.
        #[derive(Debug)]
        struct BossCount(usize);
        impl ProgressionAxis for BossCount {
            fn name(&self) -> &str {
                "boss_count"
            }
            fn value(&self, _: &AxisInput<'_>) -> f64 {
                self.0 as f64
            }
        }

        let mut book = AxisBook::with_builtins();
        book.add(Box::new(BossCount(2))).unwrap();

        let curves = CurveBook::new().with(CurveTable::new(
            AssetPath::new("/Content/Curves/b.cvcurve").unwrap(),
            "boss_count",
        ));
        assert!(book.check(&curves).is_ok(), "the lint is satisfied now");

        let (g, reaches) = world();
        assert_eq!(book.value("boss_count", &input(&g, reaches[0])), Some(2.0));
    }

    #[test]
    fn a_name_resolves_to_exactly_one_axis() {
        // ⚠ Two axes named `depth` would make *"read over depth"* mean whichever loaded last, and
        // nothing in the table would say which.
        let mut book = AxisBook::with_builtins();
        assert!(matches!(
            book.add(Box::new(Depth)),
            Err(AxisError::Duplicate { .. })
        ));
        assert_eq!(book.len(), 4);
    }

    #[test]
    fn the_builtins_are_the_four_the_core_can_compute_from_the_graph() {
        let book = AxisBook::with_builtins();
        assert_eq!(
            book.names().collect::<Vec<_>>(),
            vec!["depth", "space_count", "sphere", "unlock_count"]
        );
    }

    #[test]
    fn an_axis_is_not_clamped_because_a_counted_axis_has_no_upper_bound() {
        // ⚠ Clamping to 0..1 here would silently flatten every counted axis. A row is clamped to its
        // own keys instead, which is where the author drew them.
        let (g, reaches) = world();
        let mut i = input(&g, reaches[0]);
        i.unlocks_held = 40;
        assert_eq!(UnlockCount.value(&i), 40.0);
    }

    #[test]
    fn the_lint_passes_when_every_loaded_table_has_its_axis() {
        let curves = CurveBook::new()
            .with(CurveTable::new(
                AssetPath::new("/Content/Curves/a.cvcurve").unwrap(),
                "depth",
            ))
            .with(CurveTable::new(
                AssetPath::new("/Content/Curves/b.cvcurve").unwrap(),
                "sphere",
            ));
        assert!(AxisBook::with_builtins().check(&curves).is_ok());
    }

    #[test]
    fn an_axis_reads_a_realized_graph_the_same_as_a_projected_one() {
        // Axes describe *position*, and a scope's position does not depend on how far its geometry
        // has been built.
        let (mut g, reaches) = world();
        let before = Depth.value(&input(&g, reaches[2]));
        g.advance(reaches[2], NodeState::Realized).ok();
        assert_eq!(Depth.value(&input(&g, reaches[2])), before);
    }
}
