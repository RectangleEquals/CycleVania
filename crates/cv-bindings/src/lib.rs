//! cv-bindings — the host-facing surface, built from one source into a native Node addon (napi-rs v3)
//! and a WASM module (wasm-bindgen), feature-gated so neither target drags in the other's toolchain.
//! The two `version` exports are cfg-gated to mutually exclusive targets, so they never coexist.
//!
//! # One source, two targets
//!
//! ⚠ **The surface is plain Rust, and the bindings are a thin wrapper over it.** A surface written
//! directly against `#[napi]` would be a surface only one target has, and *"generated from the same
//! manifest, so a member added to one appears in the other or the build fails"* would be a hope rather
//! than a property.
//!
//! * [`dials`] — `list` · `get` · `set` · `setSource`, the interface the editor panel and a shipped
//!   game both drive. ⚠ **The editor gets no private channel.**
//! * [`project`] — load, validate, seed, generate.

pub mod content;
pub mod dials;
pub mod project;
pub mod stategraph;
pub mod tables;

pub use dials::{DialBounds, DialError, DialKind, DialMeta, DialSource, DialValue, Dials};
pub use project::{GenerateOptions, Project, ProjectError};

/// Target-agnostic implementation shared by both bindings.
fn core_version() -> String {
    format!(
        "cyclevania {} (core {}, determinism {})",
        env!("CARGO_PKG_VERSION"),
        cv_core::version(),
        cv_determinism::version(),
    )
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// The bindings. Both wrap the same surface; neither contains logic.
//
// ⚠ **A wrapper that computed anything would be a place the two targets could disagree**, and
// cross-target determinism is the property this whole project is arranged around. So each of these is
// a call-through and a type conversion, and the test suite exercises the surface rather than the
// wrappers — because there is nothing in a wrapper to test that the surface does not already say.
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// The dial ids a project declares, as text the bindings can carry without a struct conversion.
///
/// ⚠ **The seam forbids closures and trait objects**, so a host walks ids and asks for each meta
/// rather than receiving a callback per dial. That is a binding-contract consequence, not a style
/// choice.
fn open(path: String, cooked: bool) -> Project {
    if cooked {
        Project::load_from_file(path)
    } else {
        Project::new(path)
    }
}

fn dial_ids(project: &Project) -> Vec<String> {
    project
        .dials()
        .list()
        .iter()
        .map(|d| d.id.clone())
        .collect()
}

/// One dial rendered as text, for a binding that has no struct conversion yet.
/// JSON-escape a string.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// One dial, as JSON.
///
/// WARN **JSON, because one of the fields is prose.** This was tab-separated and carried six fields;
/// `DialMeta` has eight, and the two it dropped were **`doc` and `bounds`** — which the plan names
/// among the five *"the editor panel renders"*. A doc containing a tab or a newline would also have
/// silently split a row into nonsense, so the carrier was wrong twice.
///
/// RARR **The panel and the host API are one surface.** A field the panel needs and the binding does not
/// carry is the shape of a private channel, which [`11-host.md`] §9.1 forbids.
fn dial_json(project: &Project, id: &str) -> String {
    match project.dials().get(id) {
        Ok(d) => {
            let b = &d.bounds;
            let num = |v: Option<f64>| v.map_or("null".into(), |x| format!("{x}"));
            let values: Vec<String> = b
                .enum_values
                .iter()
                .map(|v| format!("\"{}\"", esc(v)))
                .collect();
            format!(
                concat!(
                    "{{\"id\":\"{}\",\"owner\":\"{}\",\"kind\":\"{}\",\"doc\":\"{}\",",
                    "\"default\":\"{}\",\"effective\":\"{}\",\"source\":\"{}\",",
                    "\"overridden\":{},\"outOfBounds\":{},",
                    "\"bounds\":{{\"min\":{},\"max\":{},\"softMin\":{},\"hardMax\":{},",
                    "\"enumPath\":{},\"enumValues\":[{}]}}}}"
                ),
                esc(&d.id),
                esc(&d.owner),
                d.kind,
                esc(&d.doc),
                esc(&format!("{:?}", d.default)),
                esc(&format!("{:?}", d.effective)),
                d.source,
                d.is_overridden(),
                d.is_out_of_bounds(),
                num(b.min),
                num(b.max),
                num(b.soft_min),
                num(b.hard_max),
                b.enum_path
                    .as_ref()
                    .map_or("null".into(), |p| format!("\"{}\"", esc(p))),
                values.join(","),
            )
        }
        Err(e) => format!("{{\"error\":\"{}\"}}", esc(&e.to_string())),
    }
}

// --- Native Node addon (napi-rs v3) ---
#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
use napi_derive::napi;

#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
#[napi]
pub fn version() -> String {
    core_version()
}

/// Every dial id a project declares.
///
/// ⚠ **Ids and text, not handles.** A `Project` handle across the seam needs a napi class and a WASM
/// `JsValue` conversion that disagree about ownership; ids are the same on both targets and cost
/// nothing. The handle-shaped surface arrives with the world descriptor it would carry.
#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
#[napi]
pub fn list_dials(path: String, cooked: bool) -> Vec<String> {
    dial_ids(&open(path, cooked))
}

/// One dial, as JSON — every field `DialMeta` carries, including its **doc** and **bounds**.
#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
#[napi]
pub fn describe_dial(path: String, cooked: bool, id: String) -> String {
    dial_json(&open(path, cooked), &id)
}

/// **The project handle.**
///
/// ⚠ **The ownership disagreement is the whole reason this took a milestone.** napi wants a class it
/// owns and hands back by reference; `wasm_bindgen` wants a value it can move through a `JsValue`. The
/// resolution is that **neither owns the surface** — [`project::Project`] is plain Rust, and each target
/// gets a thin wrapper of its own. A surface written directly against one of them would be a surface
/// only one target has.
#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
#[napi(js_name = "Project")]
pub struct JsProject {
    inner: project::Project,
}

#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
#[napi]
impl JsProject {
    /// Open a project from its `.cvproj`.
    #[napi(factory)]
    pub fn open(path: String) -> napi::Result<Self> {
        project::Project::open(&path)
            .map(|inner| JsProject { inner })
            .map_err(to_napi)
    }

    /// Open a cooked package. ⚠ **One file, and the whole surface a shipped game needs.**
    #[napi(factory)]
    pub fn load_from_file(path: String) -> Self {
        JsProject {
            inner: project::Project::load_from_file(path),
        }
    }

    /// Create a new project, optionally copying an existing one's content.
    #[napi(factory)]
    pub fn create(at: String, from: Option<String>) -> napi::Result<Self> {
        let source = match from {
            Some(path) => Some(project::Project::open(&path).map_err(to_napi)?),
            None => None,
        };
        project::Project::create(&at, source.as_ref().and_then(project::Project::descriptor))
            .map(|inner| JsProject { inner })
            .map_err(to_napi)
    }

    /// Every content file, relative to the content root, sorted.
    #[napi]
    pub fn content(&self) -> Vec<String> {
        self.inner.content()
    }

    /// Read one content file.
    #[napi]
    pub fn read(&self, rel: String) -> napi::Result<String> {
        self.inner.read(&rel).map_err(to_napi)
    }

    /// Write one content file **canonically**, returning what was written.
    #[napi]
    pub fn write(&mut self, rel: String, src: String) -> napi::Result<String> {
        self.inner.write(&rel, &src).map_err(to_napi)
    }

    /// Check the project.
    #[napi]
    pub fn validate(&mut self) -> napi::Result<()> {
        self.inner.validate().map_err(to_napi)
    }

    /// Every dial id it declares, sorted.
    #[napi]
    pub fn dials(&self) -> Vec<String> {
        dial_ids(&self.inner)
    }

    /// One dial, as JSON.
    #[napi]
    pub fn dial(&self, id: String) -> String {
        dial_json(&self.inner, &id)
    }

    /// The recipe. ⚠ **Dials are part of it and the seed is not.**
    #[napi]
    pub fn fingerprint(&self) -> String {
        format!("{:016x}", self.inner.fingerprint())
    }

    /// Generate, returning the world's fingerprint and seed.
    ///
    /// ▶ **The descriptor itself does not cross here.** Reading structure out of a world is M21's
    /// shape and arrives with it; this milestone is *"the editor can work"*, not the whole host API.
    #[napi]
    pub fn generate(&self, seed: String) -> napi::Result<String> {
        self.inner
            .generate(project::GenerateOptions::seeded(seed))
            .map(|w| format!("{:016x}\t{}\t{}", w.fingerprint, w.seed, w.scopes))
            .map_err(to_napi)
    }
}

#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
fn to_napi(e: project::ProjectError) -> napi::Error {
    napi::Error::from_reason(e.to_string())
}

/// May a copied fragment paste into a document of that format?
///
/// ⚠ **A free function, not a method on the handle.** A paste is checked before anything is open —
/// a developer copies from one project and pastes into another — so requiring a `Project` would make
/// the common case the awkward one.
#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
#[napi]
pub fn may_paste(fragment: String, into: String) -> napi::Result<bool> {
    match content::may_paste(&fragment, &into) {
        Ok(()) => Ok(true),
        Err(e) => Err(napi::Error::from_reason(e.to_string())),
    }
}

/// Check a `.cvstate` document — the graph, its positions and its findings, as JSON.
///
/// ⚠ **The editor draws this and computes none of it.** The un-softlockable check over a state graph
/// is the solver's own analysis; a check is not a view.
#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
#[napi]
pub fn check_state_graph(text: String) -> napi::Result<String> {
    stategraph::check(&text).map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Read a `.cvcurve` and sample every row for drawing.
#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
#[napi]
pub fn read_curves(path: String, text: String) -> napi::Result<String> {
    tables::curves(&path, &text).map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Read a `.cvunlock` for the table view — rows, and whatever stops them building.
///
/// ⚠ **The rows come back even when the table will not build**, so a `supersedes` cycle is shown
/// *in the table* rather than deferred to a build error.
#[cfg(all(feature = "napi-addon", not(target_arch = "wasm32")))]
#[napi]
pub fn read_unlocks(text: String) -> napi::Result<String> {
    tables::unlocks(&text).map_err(|e| napi::Error::from_reason(e.to_string()))
}

// --- WASM module (wasm-bindgen) ---
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
use wasm_bindgen::prelude::wasm_bindgen;
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
use wasm_bindgen::JsValue;

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen]
pub fn version() -> String {
    core_version()
}

/// Every dial id a project declares.
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen]
pub fn list_dials(path: String, cooked: bool) -> Vec<String> {
    dial_ids(&open(path, cooked))
}

/// One dial, as JSON.
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen]
pub fn describe_dial(path: String, cooked: bool, id: String) -> String {
    dial_json(&open(path, cooked), &id)
}

/// **The project handle**, the WASM half.
///
/// ⚠ **The same plain-Rust surface, a different wrapper.** Errors cross as `JsValue` strings because
/// `wasm_bindgen` has no error type of its own to map onto — which is precisely the disagreement with
/// napi that kept a handle from crossing at all until this milestone.
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = "Project")]
pub struct JsProject {
    inner: project::Project,
}

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen(js_class = "Project")]
impl JsProject {
    /// Open a project from its `.cvproj`.
    pub fn open(path: String) -> Result<JsProject, JsValue> {
        project::Project::open(&path)
            .map(|inner| JsProject { inner })
            .map_err(to_js)
    }

    /// Open a cooked package.
    pub fn load_from_file(path: String) -> JsProject {
        JsProject {
            inner: project::Project::load_from_file(path),
        }
    }

    /// Create a new project, optionally copying an existing one's content.
    pub fn create(at: String, from: Option<String>) -> Result<JsProject, JsValue> {
        let source = match from {
            Some(path) => Some(project::Project::open(&path).map_err(to_js)?),
            None => None,
        };
        project::Project::create(&at, source.as_ref().and_then(project::Project::descriptor))
            .map(|inner| JsProject { inner })
            .map_err(to_js)
    }

    /// Every content file, relative to the content root, sorted.
    pub fn content(&self) -> Vec<String> {
        self.inner.content()
    }

    /// Read one content file.
    pub fn read(&self, rel: String) -> Result<String, JsValue> {
        self.inner.read(&rel).map_err(to_js)
    }

    /// Write one content file canonically, returning what was written.
    pub fn write(&mut self, rel: String, src: String) -> Result<String, JsValue> {
        self.inner.write(&rel, &src).map_err(to_js)
    }

    /// Check the project.
    pub fn validate(&mut self) -> Result<(), JsValue> {
        self.inner.validate().map_err(to_js)
    }

    /// Every dial id it declares, sorted.
    pub fn dials(&self) -> Vec<String> {
        dial_ids(&self.inner)
    }

    /// One dial, as JSON.
    pub fn dial(&self, id: String) -> String {
        dial_json(&self.inner, &id)
    }

    /// The recipe.
    pub fn fingerprint(&self) -> String {
        format!("{:016x}", self.inner.fingerprint())
    }

    /// Generate, returning the world's fingerprint and seed.
    pub fn generate(&self, seed: String) -> Result<String, JsValue> {
        self.inner
            .generate(project::GenerateOptions::seeded(seed))
            .map(|w| format!("{:016x}\t{}\t{}", w.fingerprint, w.seed, w.scopes))
            .map_err(to_js)
    }
}

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
fn to_js(e: project::ProjectError) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// May a copied fragment paste into a document of that format?
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen]
pub fn may_paste(fragment: String, into: String) -> Result<bool, JsValue> {
    content::may_paste(&fragment, &into)
        .map(|()| true)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Check a `.cvstate` document — the graph, its positions and its findings, as JSON.
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen]
pub fn check_state_graph(text: String) -> Result<String, JsValue> {
    stategraph::check(&text).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Read a `.cvcurve` and sample every row for drawing.
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen]
pub fn read_curves(path: String, text: String) -> Result<String, JsValue> {
    tables::curves(&path, &text).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Read a `.cvunlock` for the table view.
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen]
pub fn read_unlocks(text: String) -> Result<String, JsValue> {
    tables::unlocks(&text).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dials::{DialBounds, DialMeta, DialValue};

    #[test]
    fn the_version_names_every_layer_it_was_built_from() {
        let v = core_version();
        assert!(v.starts_with("cyclevania "));
        assert!(v.contains("core ") && v.contains("determinism "));
    }

    #[test]
    fn a_host_walks_ids_rather_than_receiving_a_callback_per_dial() {
        // ⚠ The seam forbids closures and trait objects; this is what that costs and what it buys.
        let mut p = Project::new("./game.cvproj");
        p.dials_mut().declare(DialMeta::authored(
            "/Content/Items/Hookshot",
            "length",
            DialValue::Number(30.0),
            DialBounds::number(8.0, 200.0),
        ));
        assert_eq!(dial_ids(&p), vec!["Hookshot.length".to_string()]);
        assert!(open("./game.cvproj".into(), false).cooked.eq(&false));
        assert!(open("./build/game.cvpak".into(), true).cooked);

        let line = dial_json(&p, "Hookshot.length");
        assert!(line.contains("NUMBER"));
        assert!(line.contains("AUTHORED"));
        // ⚠ The error is a JSON object now, not an `error	` prefix — a caller that
        // string-matched the old shape would silently stop noticing failures.
        assert!(dial_json(&p, "ghost.x").contains("\"error\""));
        // And the two fields the tab-separated line used to drop.
        assert!(line.contains("\"doc\"") && line.contains("\"bounds\""));
    }
}
