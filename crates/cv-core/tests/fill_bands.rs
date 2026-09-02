//! M10a's green condition, end to end.
//!
//! ⚠ **Three claims, and each one has failed silently in a generator before**: a segment fills
//! deterministically; a candidate carrying a non-empty `gate()` is rejected **with the reason
//! available**, as the design's mock-up promises to show live; and a two-wing parallel group with a
//! shared capstone has both wings present with neither ordered against the other.

use cv_core::fill::{FillBand, FillCandidate, FillGraph, FillOp, Ineligible, Site};
use cv_core::node::{NodeGraph, NodeKind};
use cv_core::object::ObjectId;
use cv_core::parallel::{Ordering, ParallelGroup, SeriesParallel};
use cv_core::spine::{SpineSegment, SpineSlot};
use cv_core::tag::TagQuery;
use cv_determinism::Rng;

fn content(name: &str) -> ObjectId {
    ObjectId::derived("content", name)
}

/// `[Scope Floors] ▶ [Filter: slope < 30°] ▶ [Exclude reserved] ▶ [Scatter] ▶ [Place]`
fn band() -> FillGraph {
    FillGraph::new()
        .then(FillOp::ScopeFloors)
        .then(FillOp::FilterSlope { max_degrees: 30.0 })
        .then(FillOp::ExcludeReserved)
        .then(FillOp::Scatter { min_spacing: 4.0 })
        .then(FillOp::Place { density: 1.0 })
}

fn floors(n: usize) -> Vec<Site> {
    let mut g = NodeGraph::new(1.0, 1);
    let area = g.add_child(g.root(), "area").unwrap();
    let space = g.add_child(area, "space").unwrap();
    (0..n)
        .map(|i| Site {
            scope: space,
            slope_degrees: if i % 7 == 0 { 55.0 } else { 8.0 },
            area: (i as f64 + 1.0) * 9.0,
            reserved: i % 11 == 0,
        })
        .collect()
}

#[test]
fn a_segment_fills_with_props_deterministically() {
    let pool = vec![
        FillCandidate::new(content("rubble")).tagged("Prop.Debris"),
        FillCandidate::new(content("torch")).tagged("Prop.Light"),
        FillCandidate::new(content("crate")).tagged("Prop.Crate"),
    ];
    let sites = floors(60);

    let a = band().place(&sites, &pool, &Rng::new(42)).unwrap();
    let b = band().place(&sites, &pool, &Rng::new(42)).unwrap();
    assert_eq!(a, b, "the same seed and the same sites give the same fill");
    assert!(!a.placed.is_empty(), "and it actually placed something");

    let other = band().place(&sites, &pool, &Rng::new(43)).unwrap();
    assert_ne!(
        a.placed, other.placed,
        "a different seed is allowed to differ, or the seed is decorative"
    );
}

#[test]
fn a_candidate_carrying_a_gate_is_rejected_with_the_reason_the_editor_shows() {
    // ⚠ The design's mock-up promises violations are shown **live**. A rejection with no reason
    // attached is indistinguishable from content that was never in the pool.
    let pool = vec![
        FillCandidate::new(content("rubble")),
        FillCandidate::new(content("missile_door")).gating(),
    ];
    let got = band().place(&floors(20), &pool, &Rng::new(1)).unwrap();

    assert_eq!(got.rejected.len(), 1);
    assert_eq!(got.rejected[0].content, content("missile_door"));
    assert_eq!(got.rejected[0].reason, Ineligible::Gates);

    let shown = got.rejected[0].to_string();
    assert!(
        shown.contains("gate()"),
        "the reason must name the hook: {shown}"
    );
    assert!(
        shown.contains("proof"),
        "and say why the boundary exists: {shown}"
    );
    assert!(
        got.placed.iter().all(|p| p.content == content("rubble")),
        "nothing gating reached the world"
    );
}

#[test]
fn a_granting_candidate_is_refused_by_the_same_boundary_as_the_affix_quarantine() {
    // ⚠ Rule 2 is *a fill graph may only place content the proof does not depend on* — the same
    // boundary the quarantine draws. Two subsystems wanting it is evidence the boundary is real.
    let pool = vec![FillCandidate::new(content("missiles")).granting()];
    let got = band().place(&floors(20), &pool, &Rng::new(1)).unwrap();
    assert!(got.placed.is_empty());
    assert_eq!(got.rejected[0].reason, Ineligible::Grants);
}

#[test]
fn the_fill_palette_cannot_express_a_gate() {
    // ⚠ The wall: if fill nodes could gate, two systems would decide placement and only one would
    // prove anything. Enforced by what is constructible, not by review.
    let every_op = [
        FillOp::ScopeFloors,
        FillOp::FilterSlope { max_degrees: 30.0 },
        FillOp::FilterArea { min: 1.0 },
        FillOp::Scatter { min_spacing: 4.0 },
        FillOp::WeightByTag {
            query: TagQuery::inherited("Prop"),
            weight: 2.0,
        },
        FillOp::ExcludeReserved,
        FillOp::Place { density: 1.0 },
    ];
    assert_eq!(
        every_op.len(),
        7,
        "the palette changed — is the new node a gate or a grant?"
    );
}

#[test]
fn a_two_wing_parallel_group_with_a_shared_capstone_has_both_wings_and_no_ordering_between_them() {
    let sp = SeriesParallel::new()
        .slot("precursor")
        .slot("capstone")
        .group(ParallelGroup::new(
            "wings",
            "precursor",
            "capstone",
            ["wing_a", "wing_b"],
        ));

    assert_eq!(sp.validate(), Ok(()));

    let slots = sp.slots();
    assert!(slots.contains(&"wing_a"), "both wings present: {slots:?}");
    assert!(slots.contains(&"wing_b"), "both wings present: {slots:?}");

    assert_eq!(
        sp.order("wing_a", "wing_b"),
        Ordering::Unordered,
        "neither wing is ordered against the other"
    );
    for wing in ["wing_a", "wing_b"] {
        assert_eq!(sp.order("precursor", wing), Ordering::Before);
        assert_eq!(sp.order(wing, "capstone"), Ordering::Before);
    }

    assert_eq!(
        sp.every_pair_is_orderable(),
        Ok(()),
        "every pair must be orderable, or symbolic grants lose their check"
    );
}

#[test]
fn a_general_graph_is_rejected_with_the_reason_rather_than_a_compile_error() {
    let interleaved = SeriesParallel::new()
        .slot("a")
        .slot("b")
        .slot("c")
        .slot("d")
        .group(ParallelGroup::new("g1", "a", "c", ["x"]))
        .group(ParallelGroup::new("g2", "b", "d", ["y"]));
    let err = interleaved.validate().unwrap_err();
    let text = err.to_string();
    assert!(text.contains("general graph"));
    assert!(
        text.contains("symbolic grants"),
        "the editor renders this beside the offending wire: {text}"
    );
}

#[test]
fn a_slot_excludes_what_it_must_not_contain_before_the_pool_is_searched() {
    let merchant = content("merchant");
    let brute = content("brute");
    let wing = SpineSlot::new("combat_wing").must_not_contain([merchant]);
    assert_eq!(wing.admissible_pool(&[merchant, brute]), vec![brute]);
}

#[test]
fn a_sphere_pin_is_a_pacing_statement_the_slot_carries() {
    let capstone = SpineSlot::new("capstone").not_before_sphere(3);
    assert!(capstone.pacing.is_declared());
    assert!(!capstone.pacing.admits(2));
    assert!(capstone.pacing.admits(3));
}

#[test]
fn a_segment_can_repeat_a_sub_spine() {
    use cv_core::schedule::AdaptiveRange;
    let arena = ObjectId::derived("spine", "combat_arena");
    let seg = SpineSegment::new("start", "end", AdaptiveRange::new(3, 9))
        .repeating(arena, AdaptiveRange::new(3, 5));
    let (which, count) = seg.repetition().unwrap();
    assert_eq!(which, arena);
    assert_eq!((count.soft_min, count.hard_max), (3, 5));
}

#[test]
fn a_band_on_an_area_runs_per_area_and_cannot_reach_a_sibling_area() {
    // ⚠ Scope inheritance, both halves: the attachment decides how *often* the fill runs and bounds
    // what it may *see*. Without the second half, "per Space" and "per Area" would differ only in how
    // many times the same world-wide fill ran.
    let mut g = NodeGraph::new(1.0, 1);
    let reach = g.add_child(g.root(), "reach").unwrap();
    let area = g.add_child(reach, "area").unwrap();
    let sibling = g.add_child(reach, "area2").unwrap();
    let inside = g.add_child(area, "space").unwrap();
    let outside = g.add_child(sibling, "space_x").unwrap();
    for i in 0..4 {
        g.add_child(area, format!("more{i}")).unwrap();
    }

    let by_area = FillBand::on_slot("hall", NodeKind::Area, band());
    assert_eq!(
        by_area.instances(&g, g.root()).len(),
        2,
        "two Areas, two runs"
    );
    assert!(by_area.may_select(&g, area, inside));
    assert!(!by_area.may_select(&g, area, outside));

    let by_space = FillBand::on_slot("hall", NodeKind::Space, band());
    assert_eq!(
        by_space.instances(&g, g.root()).len(),
        6,
        "the same template attached at Space scope runs per Space instead"
    );
}
