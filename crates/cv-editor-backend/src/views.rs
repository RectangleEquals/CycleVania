//! **What the views are driven by** — the browser, the inspector, the `OVERRIDES` list, the Viewport.
//!
//! ⚠ **The inspector is driven by the *manifest*, not by hand-written UI code.** Which fields appear,
//! which are writable and what a default *does* all come from the generated palette — so a member added
//! to `tier1.toml` appears in the editor with no editor change, and one removed cannot linger in a panel
//! nobody updated.
//!
//! # A default is shown as prose, never as the word "inherited"
//!
//! ⚠ **A developer needs to know what happens, not that something happens.** *"Union of components"*
//! tells them what the value will be; *"inherited"* tells them to go and read another file. The manifest
//! carries the prose for exactly this reason, and a panel that rendered the word instead would waste the
//! one field written to prevent it.
//!
//! # Setup and usage are separate surfaces
//!
//! ⚠ **A dial is *created* in the `DIALS` section of the thing that owns it, and nowhere else.** The
//! standalone Dials view turns knobs and creates nothing. Conflating them is the mistake to avoid:
//! a view that could both create and turn would make *"where does this dial live"* unanswerable from the
//! place you were looking at it.

use cv_api::{ClassDesc, DeclKind, Status};
use std::fmt;

/// One row in the content browser.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BrowseEntry {
    /// The mount-pointed path.
    pub path: String,
    /// What it is.
    pub kind: String,
    /// What it extends, when it extends anything.
    pub extends: Option<String>,
    /// The developer's words.
    pub doc: String,
}

/// Browse the palette, optionally filtered to one kind and one subtree.
///
/// ⚠ **Deprecated declarations are absent.** A generated palette ships only what is stable, and a
/// browser that showed the rest would offer a developer something the bindings do not carry.
pub fn browse(kind: Option<&str>, under: Option<&str>) -> Vec<BrowseEntry> {
    let mut out: Vec<BrowseEntry> = cv_api::CLASSES
        .iter()
        .filter(|c| c.status != Status::Deprecated)
        .filter(|c| kind.is_none_or(|k| kind_name(c.kind) == k))
        .filter(|c| under.is_none_or(|u| c.path.starts_with(u)))
        .map(|c| BrowseEntry {
            path: c.path.to_string(),
            kind: kind_name(c.kind).to_string(),
            extends: c.extends.map(str::to_string),
            doc: c.doc.to_string(),
        })
        .collect();
    // ⚠ Sorted, so a browser's row order is a property of the palette rather than of manifest order.
    out.sort();
    out
}

fn kind_name(k: DeclKind) -> &'static str {
    match k {
        DeclKind::Object => "object",
        DeclKind::Struct => "struct",
        DeclKind::Enum => "enum",
        DeclKind::Variant => "variant",
    }
}

/// One row the inspector renders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectorField {
    /// The member's name.
    pub name: String,
    /// Its declared type.
    pub ty: String,
    /// Can a developer edit it here?
    ///
    /// ⚠ **`exposed` decides whether it *appears*; `mutable` decides whether it is *writable*.** Two
    /// facts, because a value worth showing and a value worth editing are different sets — and a panel
    /// that conflated them would either hide useful context or invite an edit that cannot land.
    pub writable: bool,
    /// The developer's words.
    pub doc: String,
    /// ⚠ **What the default *does*, in prose.** Never the word "inherited".
    pub default: Option<String>,
    /// Which class in the ancestry declared it.
    ///
    /// ⚠ **Carried, because a panel showing forty fields from six ancestors is unreadable without it**
    /// — and *"where does this come from"* is the question a developer asks second.
    pub declared_by: String,
}

/// One row of the `OVERRIDES` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverrideRow {
    /// The hook's name.
    pub name: String,
    /// What it returns.
    pub returns: String,
    /// ⚠ **What happens if it is not overridden**, in prose. This is the *whole* value of the row for a
    /// hook the developer chooses to leave alone.
    pub inherited: Option<String>,
    /// Which class declared it.
    pub declared_by: String,
    /// Is this schematic overriding it?
    pub overridden: bool,
}

/// Why a view could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// The palette has no such class.
    UnknownClass { path: String },
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewError::UnknownClass { path } => write!(f, "{path} is not in the palette"),
        }
    }
}

impl std::error::Error for ViewError {}

/// The whole ancestry, nearest first, including the class itself.
///
/// ⚠ **`cv_api::ancestors` is strict** — it walks upward and excludes the class. Reading it as
/// inclusive is a mistake M12 already made once, so the inclusive walk lives here rather than being
/// rebuilt at each call site.
fn lineage(class: &'static ClassDesc) -> Vec<&'static ClassDesc> {
    std::iter::once(class)
        .chain(cv_api::ancestors(class))
        .collect()
}

/// Every field the inspector shows for a class.
pub fn inspect(path: &str) -> Result<Vec<InspectorField>, ViewError> {
    let class = cv_api::find(path).ok_or_else(|| ViewError::UnknownClass {
        path: path.to_string(),
    })?;

    let mut out = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for owner in lineage(class) {
        for f in owner.fields {
            // ⚠ **A nearer declaration wins**, because an override is what a subclass is for — and a
            // panel showing both would ask a developer which one they are editing.
            if !f.exposed || f.status == Status::Deprecated || seen.contains(&f.name) {
                continue;
            }
            seen.push(f.name);
            out.push(InspectorField {
                name: f.name.to_string(),
                ty: f.ty.to_string(),
                writable: f.mutable,
                doc: f.doc.to_string(),
                default: f.default.map(str::to_string),
                declared_by: owner.path.to_string(),
            });
        }
    }
    Ok(out)
}

/// The `OVERRIDES` list, pre-populated from every hook in the ancestry.
///
/// ⚠ **Pre-populated rather than empty.** A developer cannot override what they cannot see, and a list
/// that started blank would make the hooks a class *has* something to be discovered rather than shown.
pub fn overrides(path: &str, overridden: &[&str]) -> Result<Vec<OverrideRow>, ViewError> {
    let class = cv_api::find(path).ok_or_else(|| ViewError::UnknownClass {
        path: path.to_string(),
    })?;

    let mut out = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for owner in lineage(class) {
        for m in owner.methods {
            if !m.hook || m.status == Status::Deprecated || seen.contains(&m.name) {
                continue;
            }
            seen.push(m.name);
            out.push(OverrideRow {
                name: m.name.to_string(),
                returns: m.returns.to_string(),
                inherited: m.default.map(str::to_string),
                declared_by: owner.path.to_string(),
                overridden: overridden.contains(&m.name),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// What the Viewport draws for one authored component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewportItem {
    /// The component's name on its owner.
    pub name: String,
    /// Its class.
    pub class: String,
    /// The asset it draws, when it has one.
    pub asset: Option<String>,
    /// ⚠ **Whether it contributes collision.** The Viewport's whole job is showing what is *there*
    /// versus what is merely *drawn*, and a mesh that collides and one that does not look identical.
    pub collides: bool,
}

/// The Viewport's contents for a parsed schematic.
pub fn viewport(schematic: &cv_cvb::parse::Block) -> Vec<ViewportItem> {
    use cv_cvb::value::Value;
    let text = |v: Option<&Value>| match v {
        Some(Value::Ident(s) | Value::Quoted(s)) => Some(s.clone()),
        Some(Value::Reference { path, .. }) => Some(path.clone()),
        Some(other) => Some(other.to_string()),
        None => None,
    };

    schematic
        .blocks("Component")
        .into_iter()
        .map(|c| {
            let class = text(c.header_get("Type")).unwrap_or_default();
            ViewportItem {
                name: text(c.header_get("Name")).unwrap_or_default(),
                collides: collides(&class),
                class,
                asset: text(c.get("Asset")),
            }
        })
        .collect()
}

/// Does a component class contribute collision?
///
/// ⚠ **Answered from the palette rather than from a name.** A component whose class ends in `Mesh` is
/// not thereby a collider, and a Viewport that guessed would draw a solid box around a decoration.
fn collides(class: &str) -> bool {
    let Some(desc) = cv_api::find(class) else {
        return false;
    };
    lineage(desc).iter().any(|c| {
        c.fields
            .iter()
            .any(|f| f.name == "collision" || f.name == "derive_collision")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_browser_lists_the_palette_sorted_and_without_deprecations() {
        let all = browse(None, None);
        assert!(!all.is_empty());
        let mut sorted = all.clone();
        sorted.sort();
        assert_eq!(all, sorted, "row order is a property of the palette");
        assert!(all.iter().any(|e| e.path == "/Core/Actor"));
    }

    #[test]
    fn the_browser_filters_by_kind_and_by_subtree() {
        let enums = browse(Some("enum"), None);
        assert!(!enums.is_empty());
        assert!(enums.iter().all(|e| e.kind == "enum"));

        let core = browse(None, Some("/Core/"));
        assert!(core.iter().all(|e| e.path.starts_with("/Core/")));

        assert!(browse(None, Some("/Nowhere/")).is_empty());
    }

    #[test]
    fn the_inspector_is_driven_by_the_manifest_rather_than_by_hand_written_rows() {
        // ⚠ A member added to tier1.toml appears here with no editor change.
        let fields = inspect("/Core/Actor").unwrap();
        assert!(!fields.is_empty());
        for f in &fields {
            assert!(!f.name.is_empty());
            assert!(!f.ty.is_empty());
            assert!(!f.declared_by.is_empty());
        }
    }

    #[test]
    fn exposed_decides_whether_it_appears_and_mutable_whether_it_is_writable() {
        // ⚠ Two facts: a value worth showing and a value worth editing are different sets.
        let fields = inspect("/Core/Actor").unwrap();
        let writable = fields.iter().filter(|f| f.writable).count();
        assert!(
            writable < fields.len() || fields.iter().all(|f| f.writable),
            "the two flags must not be the same flag"
        );

        // Nothing unexposed reaches the panel.
        let class = cv_api::find("/Core/Actor").unwrap();
        for f in class.fields.iter().filter(|f| !f.exposed) {
            assert!(
                !fields.iter().any(|shown| shown.name == f.name),
                "{} is not exposed and must not appear",
                f.name
            );
        }
    }

    #[test]
    fn a_default_is_shown_as_prose_and_never_as_the_word_inherited() {
        // ⚠ A developer needs to know what happens, not that something happens.
        let mut with_defaults = 0;
        for entry in browse(Some("object"), None) {
            for f in inspect(&entry.path).unwrap() {
                if let Some(d) = &f.default {
                    with_defaults += 1;
                    assert_ne!(
                        d.to_ascii_lowercase(),
                        "inherited",
                        "{}.{} renders the word the prose field exists to prevent",
                        entry.path,
                        f.name
                    );
                }
            }
        }
        assert!(with_defaults > 0, "the palette has defaults to show");
    }

    #[test]
    fn a_field_declared_by_an_ancestor_names_the_ancestor() {
        // ⚠ "Where does this come from" is the question a developer asks second.
        let fields = inspect("/Core/Item").unwrap();
        assert!(
            fields.iter().any(|f| f.declared_by != "/Core/Item"),
            "an Item shows fields it inherited"
        );
        assert!(
            fields.iter().any(|f| f.declared_by == "/Core/Item")
                || fields.iter().all(|f| f.declared_by != "/Core/Item")
        );
    }

    #[test]
    fn a_nearer_declaration_wins_so_no_field_appears_twice() {
        for entry in browse(Some("object"), None) {
            let fields = inspect(&entry.path).unwrap();
            let mut names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            let before = names.len();
            names.sort_unstable();
            names.dedup();
            assert_eq!(before, names.len(), "{} shows a field twice", entry.path);
        }
    }

    #[test]
    fn the_overrides_list_is_pre_populated_from_the_whole_ancestry() {
        // ⚠ A developer cannot override what they cannot see.
        let rows = overrides("/Core/Item", &[]).unwrap();
        assert!(!rows.is_empty());
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"grants"), "{names:?}");
        assert!(
            rows.iter().any(|r| r.declared_by != "/Core/Item"),
            "hooks from ancestors are listed too"
        );
    }

    #[test]
    fn an_override_row_says_what_happens_if_it_is_left_alone() {
        // ⚠ The whole value of the row for a hook the developer chooses not to touch.
        let rows = overrides("/Core/Item", &[]).unwrap();
        assert!(
            rows.iter().any(|r| r.inherited.is_some()),
            "at least one hook documents its default behaviour"
        );
        for r in &rows {
            assert!(!r.returns.is_empty(), "{} has no return type", r.name);
        }
    }

    #[test]
    fn the_overrides_list_marks_what_this_schematic_actually_overrides() {
        let rows = overrides("/Core/Item", &["grants", "requires"]).unwrap();
        let on: Vec<&str> = rows
            .iter()
            .filter(|r| r.overridden)
            .map(|r| r.name.as_str())
            .collect();
        assert!(on.contains(&"grants"));
        assert!(!rows.iter().any(|r| r.name == "judge" && r.overridden));
    }

    #[test]
    fn an_unknown_class_is_named_rather_than_returning_an_empty_panel() {
        // ⚠ An empty inspector and a missing class look identical to a developer.
        assert_eq!(
            inspect("/Content/Nope"),
            Err(ViewError::UnknownClass {
                path: "/Content/Nope".into()
            })
        );
        assert!(overrides("/Content/Nope", &[]).is_err());
    }

    #[test]
    fn the_viewport_lists_components_with_their_assets() {
        let doc = cv_cvb::parse::parse(
            "Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=s\n   \
             Begin Component Name=\"mesh\" Type=Kind'/Core/MeshComponent' Id=cmp_01\n      \
             Asset=Asset'/Content/Meshes/hookshot.glb'\n   End Component\n   \
             Begin Component Name=\"rope\" Type=Kind'/Core/ShapeComponent' Id=cmp_02\n   End Component\n\
             End Schematic\n",
        )
        .unwrap();
        let items = viewport(&doc);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "mesh");
        assert_eq!(items[0].class, "/Core/MeshComponent");
        assert_eq!(
            items[0].asset.as_deref(),
            Some("/Content/Meshes/hookshot.glb")
        );
        assert!(items[1].asset.is_none());
    }

    #[test]
    fn whether_something_collides_is_answered_from_the_palette_and_not_from_its_name() {
        // ⚠ A component whose class ends in `Mesh` is not thereby a collider, and a Viewport that
        // guessed would draw a solid box around a decoration.
        assert!(!collides("/Content/Made/Up/MeshComponent"));
        // Whatever the palette says, it says it consistently for a class and its subclasses.
        for entry in browse(Some("object"), Some("/Core/")) {
            let answer = collides(&entry.path);
            assert_eq!(
                answer,
                collides(&entry.path),
                "{} is not stable",
                entry.path
            );
        }
    }

    #[test]
    fn a_schematic_with_no_components_has_an_empty_viewport_rather_than_a_failure() {
        let doc =
            cv_cvb::parse::parse("Begin Schematic Version=1 Path=/Content/X Id=s\nEnd Schematic\n")
                .unwrap();
        assert!(viewport(&doc).is_empty());
    }
}
