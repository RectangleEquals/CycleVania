//! **What the Curve editor and the Unlock table draw.**
//!
//! ⚠ **The unlock table's rows must survive a fault.** The engine refuses a `supersedes` cycle before
//! building — the closure is taken once and everything downstream trusts it — but a *view* that got
//! only an error would show a message and no table, leaving the developer to find the cycle by reading
//! the file. Which is what the view exists to replace.

use cv_bindings::tables::{curves, unlocks};

const CURVE: &str = r#"
{ "version": 1,
  "domain":  "tanks",
  "y_label": "reach",
  "rows": {
    "wide": { "interpolation": "LINEAR", "points": [[0.0,0.0],[12.0,120.0]] }
  } }"#;

const CLEAN: &str = r#"{
  "version": 1,
  "unlocks": [
    { "id": "u_grapple", "name": "Grapple", "doc": "reach a ledge" },
    { "id": "u_grapple_2", "name": "Long Grapple", "supersedes": ["u_grapple"] }
  ]
}"#;

const CYCLIC: &str = r#"{
  "version": 1,
  "unlocks": [
    { "id": "a", "name": "A", "supersedes": ["b"] },
    { "id": "b", "name": "B", "supersedes": ["a"] }
  ]
}"#;

#[test]
fn a_curve_is_sampled_across_its_own_keys() {
    // ⚠ **The defect this guards.** `sample` clamps outside the keyed range, so a curve keyed over
    // `0..12` previewed across an assumed `0..1` renders as a flat line at its first value — and flat
    // is exactly what a *broken* curve looks like, so the preview would lie in the worst direction.
    // ⚠ No early return: a test that skips itself when the fixture is wrong passes by doing
    // nothing, which is the failure mode this whole file exists to rule out.
    let json = curves("/Content/Curves/reach.cvcurve", CURVE).expect("the fixture reads");
    assert!(json.contains("\"from\":0"), "{json}");
    assert!(json.contains("\"to\":12"), "{json}");
    // The last sampled y must be the far key, not the near one.
    assert!(
        json.contains("120"),
        "the preview must reach the far key: {json}"
    );
}

#[test]
fn a_clean_unlock_table_has_no_fault() {
    let json = unlocks(CLEAN).expect("a clean table reads");
    assert!(json.contains("\"fault\":null"), "{json}");
    assert!(json.contains("u_grapple_2"));
    assert!(json.contains("\"supersedes\":[\"u_grapple\"]"), "{json}");
}

#[test]
fn a_cycle_comes_back_with_the_rows_it_is_about() {
    // ⚠ **Shown in the table, not deferred to a build error.** The rows are present *and* the fault
    // names which of them form the cycle, so the view marks those rows rather than printing a
    // sentence beside a table the developer then searches by eye.
    let json = unlocks(CYCLIC).expect("the rows still read");
    assert!(json.contains("\"kind\":\"supersedes-cycle\""), "{json}");
    assert!(
        json.contains("\"id\":\"a\"") && json.contains("\"id\":\"b\""),
        "{json}"
    );
    // ⚠ **The fault carries the cycle as a *path*, `a -> b -> a`**, not a set — which is more
    // useful to read and means the view must dedupe before marking rows, or it marks `a` twice.
    let fault = json.split("\"rows\":[").nth(1).unwrap_or_default();
    assert!(fault.contains("\"a\"") && fault.contains("\"b\""), "{json}");
}

#[test]
fn the_strict_loader_still_refuses_what_the_view_shows() {
    // ⚠ **Two readers, one parser, different contracts.** The view must not soften the engine: a
    // table the solver would reject must still fail to load.
    assert!(cv_assets::tables::load_unlocks(CYCLIC).is_err());
    assert!(cv_assets::tables::load_unlocks(CLEAN).is_ok());
}

#[test]
fn a_malformed_table_is_refused_rather_than_half_drawn() {
    assert!(unlocks("{ not json").is_err());
}

#[test]
fn a_curve_reports_the_spelling_the_file_used() {
    // ⚠ **`CUBIC` in the file is `Interpolation::Smooth` in the core**, and the loader says so
    // outright. Emitting the enum's Debug name showed `SMOOTH` for a row a developer wrote `CUBIC` on
    // — a second word for one thing, on the side that has to match the error message they would hit,
    // which also names `CUBIC`.
    let json = curves("/Content/Curves/reach.cvcurve", CURVE).expect("reads");
    assert!(json.contains("\"interpolation\":\"LINEAR\""), "{json}");
    assert!(
        !json.contains("SMOOTH"),
        "the core's name must not cross the seam: {json}"
    );

    let cubic = CURVE.replace("LINEAR", "CUBIC");
    let json = curves("/Content/Curves/reach.cvcurve", &cubic).expect("reads");
    assert!(json.contains("\"interpolation\":\"CUBIC\""), "{json}");
    assert!(!json.contains("SMOOTH"), "{json}");
}
