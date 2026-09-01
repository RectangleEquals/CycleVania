//! M09 exit criteria: **the dial channel is connected to generation, not merely resolving.**
//!
//! > **Green when:** *"linear at first, opening up the deeper you go"* is expressible as a curve-driven
//! > dial, a dial set at World scope is overridable at one Area, all the kinds resolve, a pass reads
//! > each dial **exactly once** no matter how many graphs reference it, and ⚠ **an authored
//! > `cycle_density` dial measurably changes how much the generated world loops.**
//!
//! # Why the last clause is the milestone
//!
//! ⚠ **A dial that changes nothing is not a dial.** The first four clauses can all pass while the
//! channel resolves values nobody consumes — a table of names with a pretty resolver. The fifth is the
//! one that fails if the wiring is missing, which is why it is the one the milestone is named for.
//!
//! `cycle_density` is the design's own example of a legitimate dial: *"no Actor can say 'this world has
//! many loops'"*. If **that** cannot reach the solver, the category has no members.

use cv_core::axis::{AxisBook, AxisInput, ProgressionAxis};
use cv_core::curve::{CurveBook, CurveTable, Row};
use cv_core::dial::{DialBook, DialError, DialId, DialValue, ResolvedDials};
use cv_core::fingerprint::FingerprintBuilder;
use cv_core::mission::{MissionEdge, MissionGraph};
use cv_core::node::{Node, NodeGraph, NodeKind};
use cv_core::schedule::{AdaptiveRange, Curve};
use cv_core::solver::Solver;
use cv_core::{AssetPath, Handle};
use cv_determinism::Rng;

fn asset(p: &str) -> AssetPath {
    AssetPath::new(p).unwrap()
}

/// A test world, with the handles a dial test needs to point at.
struct World {
    graph: NodeGraph,
    root: Handle<Node>,
    areas: Vec<Handle<Node>>,
    spaces: Vec<Handle<Node>>,
}

/// World ▸ 3 Reaches ▸ 1 Area each ▸ 4 Spaces each.
fn world() -> World {
    let mut graph = NodeGraph::new(1.0, 4242);
    let root = graph.root();
    let mut areas = Vec::new();
    let mut spaces = Vec::new();
    for r in 0..3 {
        let reach = graph.add_child(root, format!("reach_{r}")).unwrap();
        let area = graph.add_child(reach, format!("area_{r}")).unwrap();
        for s in 0..4 {
            spaces.push(graph.add_child(area, format!("space_{r}_{s}")).unwrap());
        }
        areas.push(area);
    }
    World {
        graph,
        root,
        areas,
        spaces,
    }
}

/// "Linear at first, opening up the deeper you go."
fn progression() -> CurveBook {
    CurveBook::new().with(
        CurveTable::new(asset("/Content/Curves/progression.cvcurve"), "depth")
            .row(
                "complexity",
                Row::linear(Curve::from_points([(0.0, 0.1), (0.5, 0.3), (1.0, 1.0)])),
            )
            .row(
                "loops",
                Row::linear(Curve::from_points([(0.0, 0.0), (1.0, 1.0)])),
            ),
    )
}

fn resolve(book: &DialBook, g: &NodeGraph) -> ResolvedDials {
    ResolvedDials::resolve(book, g, &progression(), &AxisBook::with_builtins(), 0).unwrap()
}

// ---------------------------------------------------------------------------------------------
// THE criterion: an authored dial measurably changes the generated world
// ---------------------------------------------------------------------------------------------

/// A mission graph laid out as a chain over the Spaces, so shortcuts have somewhere to go.
fn chain(spaces: &[Handle<Node>]) -> MissionGraph {
    let mut m = MissionGraph::new(spaces[0]);
    for w in spaces.windows(2) {
        m.add_edge(MissionEdge::open(w[0], w[1]));
    }
    m
}

#[test]
fn an_authored_cycle_density_dial_changes_how_much_the_world_loops() {
    // ⚠ **The milestone's real green criterion.** Everything else can pass while the channel resolves
    // values nobody reads; this is the clause that fails if the wiring is missing.
    let w = world();
    let (g, root, spaces) = (&w.graph, w.root, &w.spaces);
    let id = DialId::new("World", "cycle_density");
    let rng = Rng::new(0xD1A15);

    let loops_at = |density: f64| {
        let book = DialBook::new().with(id.clone(), root, DialValue::number(density));
        let dials = resolve(&book, g);
        let mut mission = chain(spaces);
        Solver::new(g)
            .reading_dial(&dials, id.clone())
            .add_cycles(&mut mission, &rng)
    };

    let none = loops_at(0.0);
    let some = loops_at(0.5);
    let many = loops_at(1.0);

    assert_eq!(none, 0, "a dial of zero must produce a pure chain");
    assert!(
        many > some && some > none,
        "raising the dial must add loops: {none} → {some} → {many}"
    );
}

#[test]
fn a_dial_set_on_one_area_loops_that_area_and_not_the_others() {
    // ⚠ **Read at the shortcut's own scope**, which is why resolution walks the ladder at all. A
    // shortcut belongs to the scope containing *both* its ends, so a dial on one Area reaches exactly
    // the shortcuts inside it.
    let w = world();
    let (g, root, areas, spaces) = (&w.graph, w.root, &w.areas, &w.spaces);
    let id = DialId::new("Area", "cycle_density");
    let rng = Rng::new(0xA4EA);

    let book = DialBook::new()
        .with(id.clone(), root, DialValue::number(0.0))
        .with(id.clone(), areas[1], DialValue::number(1.0));
    let dials = resolve(&book, g);

    let mut mission = chain(spaces);
    let added = Solver::new(g)
        .reading_dial(&dials, id.clone())
        .add_cycles(&mut mission, &rng);
    assert!(added > 0, "the one loud Area must produce shortcuts");

    // Every shortcut must sit inside the Area that asked for one.
    let inside: Vec<Handle<Node>> = spaces[4..8].to_vec();
    for e in mission.edges().iter().filter(|e| e.is_shortcut) {
        assert!(
            inside.contains(&e.from) && inside.contains(&e.to),
            "a shortcut escaped the Area whose dial asked for it"
        );
    }
}

#[test]
fn a_project_that_authors_no_dial_still_gets_a_number() {
    // ⚠ The core ships no dials, so a project with none still needs a density. `with_cycle_density` is
    // the **fallback**, not the only way to set it — and a solver reading a dial that was never
    // authored falls back to it rather than to zero.
    let w = world();
    let (g, spaces) = (&w.graph, &w.spaces);
    let rng = Rng::new(0xFA11);

    let mut a = chain(spaces);
    let plain = Solver::new(g)
        .with_cycle_density(1.0)
        .add_cycles(&mut a, &rng);
    assert!(plain > 0);

    let dials = resolve(&DialBook::new(), g);
    let mut b = chain(spaces);
    let unauthored = Solver::new(g)
        .with_cycle_density(1.0)
        .reading_dial(&dials, DialId::new("World", "cycle_density"))
        .add_cycles(&mut b, &rng);
    assert_eq!(
        unauthored, plain,
        "an unauthored dial falls back, not to zero"
    );
}

// ---------------------------------------------------------------------------------------------
// "Linear at first, opening up the deeper you go"
// ---------------------------------------------------------------------------------------------

#[test]
fn one_authored_curve_gives_a_different_value_in_every_reach() {
    // ⚠ The shape the milestone names. Nobody wrote a value per scope — the curve is authored once and
    // read at each scope's own depth.
    let w = world();
    let (g, root, areas) = (&w.graph, w.root, &w.areas);
    let id = DialId::new("Area", "complexity");
    let book = DialBook::new().with(
        id.clone(),
        root,
        DialValue::curve(asset("/Content/Curves/progression.cvcurve"), "complexity"),
    );
    let d = resolve(&book, g);

    let values: Vec<f64> = areas.iter().map(|a| d.number(&id, *a).unwrap()).collect();
    assert_eq!(values[0], 0.1, "shallow");
    assert_eq!(values[1], 0.3, "middle");
    assert_eq!(values[2], 1.0, "deep");
    assert!(
        values[2] - values[1] > values[1] - values[0],
        "linear at first, opening up: {values:?}"
    );
}

#[test]
fn a_curve_driven_dial_reaches_the_solver_like_any_other() {
    // The two halves together: a curve supplies the number, and the number changes the world.
    let w = world();
    let (g, root, spaces) = (&w.graph, w.root, &w.spaces);
    let id = DialId::new("Area", "cycle_density");
    let rng = Rng::new(0xC0FFEE);

    let book = DialBook::new().with(
        id.clone(),
        root,
        DialValue::curve(asset("/Content/Curves/progression.cvcurve"), "loops"),
    );
    let dials = resolve(&book, g);

    let mut mission = chain(spaces);
    Solver::new(g)
        .reading_dial(&dials, id)
        .add_cycles(&mut mission, &rng);

    // The `loops` row runs 0 → 1 over depth, so how much an Area loops rises with how deep it is.
    let shortcuts: Vec<&MissionEdge> = mission.edges().iter().filter(|e| e.is_shortcut).collect();
    assert!(!shortcuts.is_empty(), "the deep end must loop");

    let count_in = |range: std::ops::Range<usize>| {
        let scope: Vec<Handle<Node>> = spaces[range].to_vec();
        shortcuts
            .iter()
            .filter(|e| scope.contains(&e.from) && scope.contains(&e.to))
            .count()
    };
    assert_eq!(
        count_in(0..4),
        0,
        "at depth 0 the curve reads 0.0, so the first Area stays a chain"
    );
    assert!(
        count_in(8..12) >= count_in(4..8),
        "and it opens up with depth: {} then {}",
        count_in(4..8),
        count_in(8..12)
    );
}

// ---------------------------------------------------------------------------------------------
// Scope resolution and provenance
// ---------------------------------------------------------------------------------------------

#[test]
fn a_world_value_is_overridable_at_one_area_and_the_trace_says_which() {
    // ⚠ *"Why is this room like this?"* is unanswerable from the number alone.
    let w = world();
    let (g, root, areas, spaces) = (&w.graph, w.root, &w.areas, &w.spaces);
    let id = DialId::new("Area", "complexity");
    let book = DialBook::new()
        .with(id.clone(), root, DialValue::number(0.25))
        .with(id.clone(), areas[2], DialValue::number(0.95));
    let d = resolve(&book, g);

    assert_eq!(d.number(&id, spaces[0]), Some(0.25));
    assert_eq!(d.number(&id, spaces[8]), Some(0.95));

    assert_eq!(d.get(&id, spaces[0]).unwrap().from_kind, NodeKind::World);
    assert_eq!(d.get(&id, spaces[8]).unwrap().from_kind, NodeKind::Area);
    assert_eq!(d.get(&id, spaces[8]).unwrap().from_scope, areas[2]);
}

#[test]
fn every_form_resolves() {
    let w = world();
    let (g, root, spaces) = (&w.graph, w.root, &w.spaces);
    let mut book = DialBook::new();
    let n = DialId::new("A", "n");
    let r = DialId::new("A", "r");
    let a = DialId::new("A", "a");
    let e = DialId::new("A", "e");
    let c = DialId::new("A", "c");
    let t = DialId::new("A", "t");

    book.set(n.clone(), root, DialValue::number(3.0));
    book.set(r.clone(), root, DialValue::range(2.0, 8.0));
    book.set(
        a.clone(),
        root,
        DialValue::adaptive(AdaptiveRange::new(4, 12)),
    );
    book.set(
        e.clone(),
        root,
        DialValue::enum_value("/Core/InstanceScope", "AREA"),
    );
    book.set(
        c.clone(),
        root,
        DialValue::curve(asset("/Content/Curves/progression.cvcurve"), "complexity"),
    );
    book.set(
        t.clone(),
        root,
        DialValue::table(asset("/Content/Curves/progression.cvcurve"), "depth"),
    );

    let d = resolve(&book, g);
    let s = spaces[0];
    for id in [&n, &r, &a, &e, &c, &t] {
        assert!(d.get(id, s).is_some(), "{id} did not resolve");
    }

    assert_eq!(d.number(&n, s), Some(3.0));
    assert_eq!(d.number(&r, s), Some(5.0), "the midpoint of a hard range");
    assert_eq!(d.clamp(&r, s, 99.0), 8.0, "and hard means hard");
    assert_eq!(d.number(&a, s), Some(12.0));
    assert_eq!(d.number(&c, s), Some(0.1));
    // ⚠ Neither of these is a number, and neither pretends to be.
    assert_eq!(d.number(&e, s), None);
    assert_eq!(d.number(&t, s), None);
}

// ---------------------------------------------------------------------------------------------
// Resolve once
// ---------------------------------------------------------------------------------------------

#[test]
fn a_pass_reads_each_dial_once_however_many_consumers_there_are() {
    // ⚠ **Structural, not conventional.** `ResolvedDials` holds no reference to the book or the axes,
    // so a consumer cannot re-read even by accident — there is nothing to re-read *from*.
    let w = world();
    let (g, root, spaces) = (&w.graph, w.root, &w.spaces);
    let id = DialId::new("World", "cycle_density");
    let book = DialBook::new().with(id.clone(), root, DialValue::number(0.5));
    let dials = resolve(&book, g);

    let scopes = g.iter().count();
    assert_eq!(
        dials.reads(),
        scopes,
        "one lookup per scope while resolving"
    );

    // Ten solvers, each walking every pair of Spaces. The read count does not move.
    let rng = Rng::new(1);
    for _ in 0..10 {
        let mut mission = chain(spaces);
        Solver::new(g)
            .reading_dial(&dials, id.clone())
            .add_cycles(&mut mission, &rng);
    }
    assert_eq!(dials.reads(), scopes, "consumers do not add reads");
}

#[test]
fn resolution_is_deterministic_across_repeats() {
    // A dial is an input to the recipe, so resolving the same book twice must give the same table —
    // otherwise the fingerprint would not describe the world.
    let w = world();
    let (g, root) = (&w.graph, w.root);
    let id = DialId::new("A", "c");
    let book = DialBook::new().with(
        id,
        root,
        DialValue::curve(asset("/Content/Curves/progression.cvcurve"), "complexity"),
    );
    assert_eq!(resolve(&book, g), resolve(&book, g));
}

// ---------------------------------------------------------------------------------------------
// Changing a dial is a different recipe
// ---------------------------------------------------------------------------------------------

#[test]
fn changing_a_dial_changes_the_fingerprint() {
    // ⚠ **Which is what makes a changed dial regenerate the world in full.** Partial regeneration is
    // not merely hard, it is *wrong*: decisions made against the old value would survive, and no seed
    // would explain the result. The fingerprint is what makes that non-negotiable rather than a rule
    // someone has to remember.
    let w = world();
    let id = DialId::new("World", "cycle_density");

    let fp = |v: f64| {
        let book = DialBook::new().with(id.clone(), w.root, DialValue::number(v));
        FingerprintBuilder::new("test").dials(&book).finish()
    };

    assert_ne!(fp(0.2), fp(0.9));
    assert_eq!(fp(0.2), fp(0.2), "and the same recipe is the same recipe");
}

#[test]
fn the_fingerprint_folds_in_the_authored_book_and_not_the_resolved_table() {
    // ⚠ A resolved table varies with the *world* — a curve dial reads a different number in every
    // Reach — so folding that in would make the fingerprint depend on its own output.
    let w = world();
    let id = DialId::new("A", "complexity");
    let book = DialBook::new().with(
        id,
        w.root,
        DialValue::curve(asset("/Content/Curves/progression.cvcurve"), "complexity"),
    );

    let one = FingerprintBuilder::new("test").dials(&book).finish();

    // A world with more Reaches resolves that dial to different numbers, and must not move the
    // fingerprint: the recipe did not change, only what it produced.
    let mut bigger = NodeGraph::new(1.0, 4242);
    let root = bigger.root();
    for r in 0..9 {
        bigger.add_child(root, format!("reach_{r}")).unwrap();
    }
    let _ = resolve(&book, &bigger);

    assert_eq!(one, FingerprintBuilder::new("test").dials(&book).finish());
}

#[test]
fn authoring_order_is_not_part_of_the_recipe() {
    // Two books with the same values authored in a different order are the same recipe.
    let w = world();
    let id = DialId::new("A", "x");
    let mut first = DialBook::new();
    first.set(id.clone(), w.root, DialValue::number(1.0));
    first.set(id.clone(), w.areas[0], DialValue::number(2.0));

    let mut second = DialBook::new();
    second.set(id.clone(), w.areas[0], DialValue::number(2.0));
    second.set(id.clone(), w.root, DialValue::number(1.0));

    assert_eq!(
        FingerprintBuilder::new("t").dials(&first).finish(),
        FingerprintBuilder::new("t").dials(&second).finish()
    );
}

// ---------------------------------------------------------------------------------------------
// Binding by name, and failing loudly
// ---------------------------------------------------------------------------------------------

#[test]
fn a_developers_own_axis_drives_a_dial_the_core_could_not_have_computed() {
    // ⚠ *"Complexity gains weight each time a boss is placed"* — the core cannot derive this, and a
    // pluggable axis is the only way to say it.
    #[derive(Debug)]
    struct BossCount(f64);
    impl ProgressionAxis for BossCount {
        fn name(&self) -> &str {
            "boss_count"
        }
        fn value(&self, _: &AxisInput<'_>) -> f64 {
            self.0
        }
    }

    let w = world();
    let (g, root, spaces) = (&w.graph, w.root, &w.spaces);
    let curves = CurveBook::new().with(
        CurveTable::new(asset("/Content/Curves/boss.cvcurve"), "boss_count").row(
            "weight",
            Row::linear(Curve::from_points([(0.0, 0.0), (4.0, 1.0)])),
        ),
    );
    let mut axes = AxisBook::with_builtins();
    axes.add(Box::new(BossCount(2.0))).unwrap();
    assert!(axes.check(&curves).is_ok(), "the axis lint is satisfied");

    let id = DialId::new("A", "weight");
    let book = DialBook::new().with(
        id.clone(),
        root,
        DialValue::curve(asset("/Content/Curves/boss.cvcurve"), "weight"),
    );
    let d = ResolvedDials::resolve(&book, g, &curves, &axes, 0).unwrap();
    assert_eq!(d.number(&id, spaces[0]), Some(0.5), "two of four bosses");
}

#[test]
fn a_dial_over_an_axis_nobody_provides_fails_before_generation() {
    // ⚠ A zero here would pin every curve to its first key, world-wide, with nothing saying why.
    let w = world();
    let (g, root) = (&w.graph, w.root);
    let curves = CurveBook::new().with(
        CurveTable::new(asset("/Content/Curves/boss.cvcurve"), "boss_count")
            .row("weight", Row::linear(Curve::constant(1.0))),
    );
    let id = DialId::new("A", "weight");
    let book = DialBook::new().with(
        id,
        root,
        DialValue::curve(asset("/Content/Curves/boss.cvcurve"), "weight"),
    );
    assert!(matches!(
        ResolvedDials::resolve(&book, g, &curves, &AxisBook::with_builtins(), 0),
        Err(DialError::UnboundDomain { .. })
    ));
}

#[test]
fn an_adaptive_dial_falls_below_its_soft_minimum_honestly() {
    // ⚠ With four eligible pieces of content and a soft minimum of ten, the answer is four — not ten
    // of the same thing, and not a build failure.
    let a = AdaptiveRange::new(10, 20);
    let w = world();
    let (g, root, spaces) = (&w.graph, w.root, &w.spaces);
    let id = DialId::new("A", "rooms");
    let book = DialBook::new().with(id.clone(), root, DialValue::adaptive(a));
    let d = resolve(&book, g);

    assert_eq!(
        d.number(&id, spaces[0]),
        Some(20.0),
        "the ceiling, if asked"
    );
    assert_eq!(
        d.clamp(&id, spaces[0], 4.0),
        4.0,
        "and scarcity is not clamped upward to the soft minimum"
    );
}
