//! **M14's green condition** — a curve-driven dial reads its row through a `ProgressionAxis`, an unlock
//! table round-trips from disk with its `supersedes` edges intact, and a renamed asset does not break a
//! reference.
//!
//! ⚠ **The fixtures are the design document's own**, copied from `09-format.md` §10. A loader tested
//! against files its author invented proves the loader agrees with itself.

use cv_assets::{digest_of, load_curves, load_unlocks, AssetId, AssetTable};
use cv_core::axis::{AxisBook, AxisInput, Depth, ProgressionAxis};
use cv_core::curve::CurveBook;
use cv_core::node::NodeGraph;
use cv_core::path::AssetPath;

/// `09-format.md` §10 — `.cvcurve`, in full.
const CURVES: &str = r#"
{ "version": 1,
  "domain":  "depth",
  "y_label": "multiplier",
  "rows": {
    "complexity":     { "interpolation": "CUBIC",  "points": [[0.0,1.0],[0.5,3.0],[1.0,6.0]] },
    "hazard_density": { "interpolation": "LINEAR", "points": [[0.0,0.1],[1.0,0.8]] }
  } }"#;

/// `09-format.md` §10 — `.cvunlock`, in full.
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

fn curve_path() -> AssetPath {
    AssetPath::new("/Content/Curves/progression.cvcurve").unwrap()
}

#[test]
fn a_curve_driven_dial_reads_its_row_through_a_progression_axis() {
    // The chain the design describes: an axis supplies `x`, the table's `domain` names that axis, and
    // the row is read at it.
    let loaded = load_curves(curve_path(), CURVES).expect("the design's own table loads");
    let mut curves = CurveBook::new();
    curves.add(loaded.table.clone());

    let axes = AxisBook::with_builtins();
    assert!(
        axes.contains("depth"),
        "the table's domain must name an axis that exists"
    );

    // ⚠ The book cross-checks that every table's `domain` has an axis — a curve read at an axis
    // nobody supplies is a dial that silently reads its first key forever.
    assert!(axes.check(&curves).is_ok());

    let mut g = NodeGraph::new(1.0, 1);
    let reach = g.add_child(g.root(), "reach").unwrap();
    let area = g.add_child(reach, "area").unwrap();

    let input = AxisInput {
        graph: &g,
        scope: area,
        unlocks_held: 0,
        sphere: None,
        sphere_count: 1,
        unlock_total: 3,
    };
    let x = axes.value("depth", &input).expect("the axis supplies an x");
    assert!((0.0..=1.0).contains(&x), "depth is normalised: {x}");

    let at_start = loaded.table.sample("complexity", 0.0).unwrap();
    let at_end = loaded.table.sample("complexity", 1.0).unwrap();
    assert_eq!(at_start, 1.0);
    assert_eq!(at_end, 6.0);

    let value = loaded.table.sample("complexity", x).unwrap();
    assert!(
        (at_start..=at_end).contains(&value),
        "the dial reads inside its authored range: {value}"
    );
    assert_eq!(Depth.name(), "depth");
}

#[test]
fn the_y_label_reaches_the_editor_and_the_generator_does_not_carry_it() {
    // ⚠ P05. It is an editor fact, so it rides on the load rather than on the core type.
    let loaded = load_curves(curve_path(), CURVES).unwrap();
    assert_eq!(loaded.y_label, "multiplier");
    assert_eq!(loaded.table.domain(), "depth");
}

#[test]
fn an_unlock_table_round_trips_from_disk_with_its_supersedes_edges_intact() {
    let table = load_unlocks(UNLOCKS).expect("the design's own table loads");

    assert_eq!(table.rows().len(), 3);
    let upgrade = table.by_id("u_2c14e8").expect("the upgrade row");
    assert_eq!(upgrade.name, "LongPullToAnchor");
    assert_eq!(upgrade.supersedes, vec!["u_7f3a91".to_string()]);

    // ⚠ The closure is what makes the edge do something: a door written for a PullToAnchor opens for a
    // LongPullToAnchor without knowing it exists.
    let base = table.by_id("u_7f3a91").unwrap().key();
    assert!(table.closure_of("u_2c14e8").contains(&base));
    assert!(
        !table.closure_of("u_7f3a91").contains(&upgrade.key()),
        "superseding is directional; the base does not satisfy its upgrade"
    );
}

#[test]
fn a_renamed_unlock_keeps_its_edges_because_supersedes_refers_by_id() {
    // ⚠ Renaming rewrites one cell.
    let renamed = UNLOCKS.replace("\"PullToAnchor\"", "\"AnchorTether\"");
    let table = load_unlocks(&renamed).unwrap();
    assert_eq!(table.by_id("u_7f3a91").unwrap().name, "AnchorTether");
    let base = table.by_id("u_7f3a91").unwrap().key();
    assert!(table.closure_of("u_2c14e8").contains(&base));
}

#[test]
fn a_renamed_asset_does_not_break_a_reference() {
    // The third of M14's green conditions, over both data resources at once.
    let mut assets = AssetTable::new();
    let curve_id = AssetId::new("a_curve_01");
    let unlock_id = AssetId::new("a_unlock_01");

    assets
        .register(
            curve_id.clone(),
            "/Content/Curves/progression.cvcurve",
            digest_of("depth: complexity, hazard_density"),
        )
        .unwrap();
    assets
        .register(
            unlock_id.clone(),
            "/Content/Progression/unlocks.cvunlock",
            digest_of("3 rows"),
        )
        .unwrap();

    let before = assets.fingerprint();

    // A developer reorganises the content root.
    assets
        .move_to(&curve_id, "/Content/Tuning/Curves/difficulty.cvcurve")
        .unwrap();
    assets
        .move_to(&unlock_id, "/Content/Tuning/unlocks.cvunlock")
        .unwrap();

    assert_eq!(
        assets.resolve(&curve_id).unwrap().path,
        "/Content/Tuning/Curves/difficulty.cvcurve",
        "the reference resolves to the new home"
    );
    assert!(assets.resolve(&unlock_id).is_ok());
    assert_eq!(
        assets.fingerprint(),
        before,
        "a move is not a content change, so the recipe is the same recipe"
    );
    assert!(
        assets.dangling([&curve_id, &unlock_id]).is_empty(),
        "nothing dangles"
    );
}

#[test]
fn a_reference_to_something_that_was_deleted_is_caught_rather_than_ignored() {
    let mut assets = AssetTable::new();
    let kept = AssetId::new("a_kept");
    assets
        .register(kept.clone(), "/Content/x.cvcurve", 1)
        .unwrap();

    let deleted = AssetId::new("a_deleted");
    let missing = assets.dangling([&kept, &deleted]);
    assert_eq!(missing, vec![&deleted]);
    assert!(assets.resolve(&deleted).is_err());
}

#[test]
fn a_mesh_is_referenced_by_its_bound_long_before_its_triangles_are_needed() {
    // ⚠ Lazy loading: the solve reasons about a path and a bound, so a large mesh costs nothing at L1.
    let mesh = cv_assets::import(
        "/Content/Meshes/quad.obj",
        b"v 0 0 0\nv 2 0 0\nv 2 3 0\nv 0 3 0\nf 1 2 3\nf 1 3 4\n",
    )
    .unwrap();
    let reference = cv_assets::MeshRef::of("/Content/Meshes/quad.obj", &mesh).unwrap();

    assert_eq!(reference.triangles, 2);
    assert_eq!(reference.bounds.extents().x, 2.0);
    assert_eq!(reference.bounds.extents().y, 3.0);

    let mut assets = AssetTable::new();
    let id = AssetId::new("a_mesh_01");
    assets
        .register(id.clone(), reference.path.clone(), digest_of("2 triangles"))
        .unwrap();
    assets.move_to(&id, "/Content/Art/quad.obj").unwrap();
    assert_eq!(assets.resolve(&id).unwrap().path, "/Content/Art/quad.obj");
}

#[test]
fn the_importable_set_is_closed_and_contains_no_spline() {
    // ⚠ **Asserted positively, because a blacklist is the weaker check** - the same lesson M10d's
    // extension boundary learned. Naming what is excluded also means naming it, and the workspace's
    // closed-set lint quite correctly refuses an undeclared `.cv*` token even inside a test asserting
    // its absence.
    //
    // ⚠ **There is deliberately no spline resource**, and this is where a reader goes looking. Every
    // use for one needs geometry that does not exist at authoring time: a developer declares a `Route`
    // and the generator produces a `Path`. *"Add a spline"* is the obvious-looking reach when M26 needs
    // a platform path, and it is the wrong move.
    for importable in ["mesh.obj", "mesh.gltf", "mesh.glb"] {
        let err = cv_assets::import(importable, b"").unwrap_err();
        assert!(
            !matches!(err, cv_assets::MeshError::UnknownFormat { .. }),
            "{importable} is an imported format, so an empty file fails for its content"
        );
    }
    for anything_else in ["path.spline", "track.curve3d", "model.fbx", "shape.svg"] {
        assert!(
            matches!(
                cv_assets::import(anything_else, b""),
                Err(cv_assets::MeshError::UnknownFormat { .. })
            ),
            "{anything_else} must not be importable"
        );
    }
}
