//! **Curves and unlock tables, as their views need them.**
//!
//! ⚠ **Sampled and inspected here, not in the editor.** A curve's shape is what `Row::sample` says it
//! is — an editor that interpolated its own preview would draw a line the generator does not follow,
//! and the drawing would be wrong in exactly the cases that matter: the ones where the interpolation
//! mode is doing something.
//!
//! ▶ **An unlock table arrives with its fault attached** rather than instead of its rows, so a
//! `supersedes` cycle can be **shown in the table** rather than deferred to a build error.

use cv_assets::tables::{inspect_unlocks, load_curves, LoadError};
use cv_core::path::AssetPath;

use crate::content::ContentError;

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn failed(e: LoadError) -> ContentError {
    ContentError::Malformed {
        rel: "<table>".into(),
        detail: e.to_string(),
    }
}

/// How many points a thumbnail is drawn from.
///
/// ⚠ **Enough to show a `Smooth` row's curvature and a `Step` row's corners.** Too few and every
/// interpolation mode draws as the same polyline, which would make the thumbnail a decoration.
const SAMPLES: usize = 48;

/// Read a `.cvcurve` and sample every row for drawing.
///
/// ⚠ **The domain comes from the file.** A curve is evaluated against an input the project names —
/// *"how many tanks"*, *"how far through the Reach"* — and a preview drawn over an assumed `0..1`
/// would be a different curve.
pub fn curves(path: &str, src: &str) -> Result<String, ContentError> {
    let asset = AssetPath::new(path).map_err(|e| ContentError::Malformed {
        rel: path.into(),
        detail: e.to_string(),
    })?;
    let loaded = load_curves(asset, src).map_err(failed)?;
    let table = &loaded.table;

    let names: Vec<String> = table.row_names().map(str::to_string).collect();
    let rows: Vec<String> = names
        .iter()
        .map(|name| {
            // ⚠ **Across the row's own keys, not an assumed `0..1`.** `sample` clamps outside the
            // keyed range, so a curve keyed over `0..12` previewed across `0..1` draws as a flat line
            // at its first value — and flat is what a *broken* curve looks like.
            let (x0, x1) = table.extent(name).unwrap_or((0.0, 1.0));
            let span = if (x1 - x0).abs() < f64::EPSILON {
                1.0
            } else {
                x1 - x0
            };
            let points: Vec<String> = (0..SAMPLES)
                .map(|i| {
                    let x = x0 + span * (i as f64 / (SAMPLES - 1) as f64);
                    let y = table.sample(name, x).unwrap_or(0.0);
                    format!("[{x:.4},{y:.4}]")
                })
                .collect();
            // ⚠ **The authored keys, not only the samples.** A polyline is a *preview*; the editor
            // Unreal ships lets a developer select a key and drag its tangent handles, and per-key
            // interpolation is already in our format — an editor that only draws the result cannot
            // author what the file can hold.
            let row = table.get(name);
            let keys: Vec<String> = row
                .map(|r| {
                    r.curve
                        .points()
                        .iter()
                        .map(|(x, y)| format!("[{x:.4},{y:.4}]"))
                        .collect()
                })
                .unwrap_or_default();
            // ⚠ **The file's vocabulary, not the enum's.** `CUBIC` in a `.cvcurve` is
            // `Interpolation::Smooth` in the core — the loader says so in as many words — so emitting
            // the Debug name taught the editor a second word for one thing, and showed `SMOOTH` for a
            // row a developer wrote `CUBIC` on. ▶ **The error they would hit names `CUBIC` too**, so
            // the authored spelling is the one that crosses.
            let interpolation = row
                .map(|r| match r.interpolation {
                    cv_core::curve::Interpolation::Linear => "LINEAR",
                    cv_core::curve::Interpolation::Step => "STEP",
                    cv_core::curve::Interpolation::Smooth => "CUBIC",
                })
                .unwrap_or("LINEAR");
            format!(
                "{{\"name\":\"{}\",\"from\":{x0},\"to\":{x1},\"interpolation\":\"{}\",\"keys\":[{}],\"points\":[{}]}}",
                esc(name),
                interpolation,
                keys.join(","),
                points.join(",")
            )
        })
        .collect();

    Ok(format!(
        "{{\"path\":\"{}\",\"domain\":\"{}\",\"yLabel\":\"{}\",\"rows\":[{}]}}",
        esc(path),
        esc(table.domain()),
        esc(&loaded.y_label),
        rows.join(",")
    ))
}

/// Read a `.cvunlock` for the table view — the rows, and whatever stops them building.
///
/// ⚠ **`id` is a column, not a per-row rule.** It is generated once and never edited, because a
/// rename that moved a key would break every reference without a migration — so the view marks the
/// whole column read-only rather than deciding row by row.
pub fn unlocks(src: &str) -> Result<String, ContentError> {
    let seen = inspect_unlocks(src).map_err(failed)?;

    let rows: Vec<String> = seen
        .rows
        .iter()
        .map(|u| {
            let sup: Vec<String> = u
                .supersedes
                .iter()
                .map(|s| format!("\"{}\"", esc(s)))
                .collect();
            format!(
                "{{\"id\":\"{}\",\"name\":\"{}\",\"doc\":\"{}\",\"supersedes\":[{}]}}",
                esc(&u.id),
                esc(&u.name),
                esc(&u.doc),
                sup.join(",")
            )
        })
        .collect();

    // ⚠ **The fault names the rows it is about**, so the view can mark them rather than print a
    // sentence beside a table the developer then has to search by eye.
    let fault = match &seen.fault {
        None => "null".to_string(),
        Some(e) => {
            let (kind, ids) = classify(e);
            format!(
                "{{\"kind\":\"{}\",\"rows\":[{}],\"message\":\"{}\"}}",
                kind,
                ids.iter()
                    .map(|i| format!("\"{}\"", esc(i)))
                    .collect::<Vec<_>>()
                    .join(","),
                esc(&e.to_string())
            )
        }
    };

    Ok(format!(
        "{{\"rows\":[{}],\"fault\":{fault}}}",
        rows.join(",")
    ))
}

/// Which rows a fault is about.
fn classify(e: &cv_core::unlock::TableError) -> (&'static str, Vec<String>) {
    use cv_core::unlock::TableError;
    match e {
        TableError::DuplicateId(id) => ("duplicate-id", vec![id.clone()]),
        TableError::UnknownSupersedes { row, missing } => {
            ("dangling-supersedes", vec![row.clone(), missing.clone()])
        }
        TableError::Cycle(ids) => ("supersedes-cycle", ids.clone()),
    }
}
