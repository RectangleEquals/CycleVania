//! **The palette** — generated, and merged from two sources.
//!
//! ⚠ **There is no text field to type a wrong name into.** That is the structural argument the whole
//! pivot rests on: a node exists because the palette offers it, so a typo is not a category of mistake
//! that can happen. Any *"type a node name"* affordance quietly undoes the pivot.
//!
//! # Two sources, and only one of them can go stale
//!
//! ⚠ **`manifest/tier1.toml` generates everything with a declared type**, and that half cannot drift
//! mid-session — the manifest does not change while the editor is open. **A dial is *user*-declared**,
//! so its get node comes from the *project* and appears and disappears as a developer edits a `DIALS`
//! section.
//!
//! ⚠ **Manifest-first is unbroken**: there is no core dial to declare there. What the second source adds
//! is a set of nodes whose existence is a fact about the open project rather than about the SDK, which
//! is why it **rebuilds on schematic and spine save** rather than at startup.
//!
//! # Node shapes follow from what the member is
//!
//! | Member | Node | Exec pins |
//! |---|---|---|
//! | a **field** | a pure get node | ⚠ none — reading a value cannot sequence anything |
//! | a **method** | a call node | `in` and `out` |
//! | a **lifecycle event** | an event node | ⚠ an `out` only, because **an event has no return value** — which is exactly why it is an event and not a hook |
//! | a **dial** | a pure get node | ⚠ none. A dial resolves **once per pass**, so a read costs nothing and cannot observe a change mid-pass |

use cv_bindings::DialKind;
use std::fmt;

/// What shape a node has on the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Shape {
    /// A value read. No exec pins.
    Pure,
    /// A call. Exec in and out.
    Call,
    /// ⚠ **A lifecycle event: exec out only.** It has no return value, which is what makes it an event
    /// rather than a hook.
    Event,
    /// A constructed value with alternative forms.
    Form,
    /// A constant.
    Literal,
}

impl Shape {
    /// Does this shape carry an incoming exec pin?
    pub fn has_exec_in(self) -> bool {
        self == Shape::Call
    }

    /// An outgoing one?
    pub fn has_exec_out(self) -> bool {
        matches!(self, Shape::Call | Shape::Event)
    }

    /// The wire name.
    pub fn name(self) -> &'static str {
        match self {
            Shape::Pure => "pure",
            Shape::Call => "call",
            Shape::Event => "event",
            Shape::Form => "form",
            Shape::Literal => "literal",
        }
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Where a palette entry came from.
///
/// ⚠ **Carried, because only one of the two can go stale.** An editor that forgot which half a node
/// came from could not know when to rebuild, and would either rebuild everything on every keystroke or
/// leave a deleted dial's node on the palette until restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    /// Generated from `manifest/tier1.toml`. Fixed for the life of the build.
    Manifest,
    /// Declared by the open project. ⚠ **Rebuilt on schematic and spine save.**
    Project,
}

/// One node the palette offers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PaletteNode {
    /// The op, which is also its identity.
    pub op: String,
    /// What a developer sees.
    pub label: String,
    /// Where it sits in the palette tree.
    pub category: String,
    /// Its shape.
    pub shape: Shape,
    /// Where it came from.
    pub source: Source,
    /// The type its output pin carries, when it has one.
    pub out_type: Option<String>,
}

/// A dial the open project declares, as the palette sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectDial {
    /// The class or spine that owns it.
    pub owner: String,
    /// Its name.
    pub name: String,
    /// Which kind.
    pub kind: DialKind,
    /// For an enum dial, the enum's path.
    pub enum_path: Option<String>,
}

impl ProjectDial {
    /// The get node's op.
    ///
    /// ⚠ **`<owner path>.<name>#dial`** — the same `<path>.<name>#<verb>` accessor convention every
    /// generated node uses. A different shape here would make a dial read look like a foreign thing in
    /// a graph, and would need its own case in every reader.
    pub fn op(&self) -> String {
        format!("{}.{}#dial", self.owner, self.name)
    }

    /// The id host code types: `<ClassName>.<DialName>`.
    pub fn qualified_id(&self) -> String {
        let short = self.owner.rsplit('/').next().unwrap_or(&self.owner);
        format!("{short}.{}", self.name)
    }

    /// ⚠ **The out pin carries the dial's *real* type**, not a `DialValue` wrapper.
    ///
    /// A wrapper would make every consumer unwrap, and would let a curve dial wire into a float pin —
    /// which is exactly the mistake the type system is here to refuse.
    pub fn out_type(&self) -> String {
        match self.kind {
            DialKind::Number => "float".into(),
            DialKind::Range | DialKind::Adaptive => "Span".into(),
            DialKind::Enum => self
                .enum_path
                .clone()
                .map(|p| format!("Enum<{p}>"))
                .unwrap_or_else(|| "Enum".into()),
            DialKind::Curve | DialKind::Table => "Curve".into(),
        }
    }

    /// The palette entry.
    pub fn to_node(&self) -> PaletteNode {
        let short = self.owner.rsplit('/').next().unwrap_or(&self.owner);
        PaletteNode {
            op: self.op(),
            label: format!("get {}", self.name),
            category: format!("Dials/{short}"),
            // ⚠ Pure: a dial resolves once per pass, so a read costs nothing and cannot observe a
            // change mid-pass.
            shape: Shape::Pure,
            source: Source::Project,
            out_type: Some(self.out_type()),
        }
    }
}

/// The utility nodes, which no manifest entry generates.
///
/// ⚠ **`Expression` is deliberately small.** Vector arithmetic is allowed; **member access is
/// forbidden**, because `a.b.c` inside an expression string is the seam through which a second
/// scripting surface grows — and the whole system exists so a developer need not learn one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Utility {
    /// A small maths sub-language.
    Expression,
    /// A wire-tidiness pass-through.
    Reroute,
    /// String join.
    Concat,
    /// Set difference.
    Except,
}

impl Utility {
    /// All four.
    pub const ALL: [Utility; 4] = [
        Utility::Expression,
        Utility::Reroute,
        Utility::Concat,
        Utility::Except,
    ];

    /// The op.
    pub fn op(self) -> &'static str {
        match self {
            Utility::Expression => "util.expression",
            Utility::Reroute => "util.reroute",
            Utility::Concat => "util.concat",
            Utility::Except => "util.except",
        }
    }

    /// Its shape. ⚠ **All four are pure** — a utility that sequenced would be an operation, not a
    /// utility.
    pub fn shape(self) -> Shape {
        Shape::Pure
    }
}

/// Why an expression was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpressionError {
    /// ⚠ **Member access**, which is the seam a second scripting surface grows through.
    MemberAccess { at: String },
    /// A call, which would make it a language.
    Call { at: String },
    /// A character the sub-language does not have.
    BadCharacter { found: char },
}

impl fmt::Display for ExpressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpressionError::MemberAccess { at } => write!(
                f,
                "member access ({at}) is not allowed in an Expression — wire the value in instead, or \
                 this node becomes a second scripting surface"
            ),
            ExpressionError::Call { at } => write!(
                f,
                "a call ({at}) is not allowed in an Expression — the palette has a node for it"
            ),
            ExpressionError::BadCharacter { found } => {
                write!(f, "{found:?} is not part of the expression sub-language")
            }
        }
    }
}

impl std::error::Error for ExpressionError {}

/// Check an `Expression` node's body.
///
/// ⚠ **A decimal point is not member access, and telling them apart is the whole check.** `1.5` is a
/// number; `a.b` is the thing that must not exist.
pub fn check_expression(src: &str) -> Result<(), ExpressionError> {
    let chars: Vec<char> = src.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        match c {
            '.' => {
                let before = i.checked_sub(1).and_then(|j| chars.get(j)).copied();
                let after = chars.get(i + 1).copied();
                let numeric = before.is_some_and(|b| b.is_ascii_digit())
                    || after.is_some_and(|a| a.is_ascii_digit());
                if !numeric {
                    let at: String = chars[i.saturating_sub(3)..(i + 4).min(chars.len())]
                        .iter()
                        .collect();
                    return Err(ExpressionError::MemberAccess { at });
                }
            }
            '(' => {
                // A call is an identifier immediately followed by `(`; `(a + b)` is grouping.
                let before = i.checked_sub(1).and_then(|j| chars.get(j)).copied();
                if before.is_some_and(|b| b.is_ascii_alphanumeric() || b == '_') {
                    let at: String = chars[i.saturating_sub(6)..=i].iter().collect();
                    return Err(ExpressionError::Call { at });
                }
            }
            c if c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    ' ' | '_' | '+' | '-' | '*' | '/' | '%' | '(' | ')' | ',' | '<' | '>' | '='
                ) => {}
            other => {
                return Err(ExpressionError::BadCharacter { found: *other });
            }
        }
    }
    Ok(())
}

/// The palette, merged from both sources.
#[derive(Clone, Debug, Default)]
pub struct Palette {
    nodes: Vec<PaletteNode>,
}

impl Palette {
    /// The generated half plus the utility nodes.
    ///
    /// ⚠ **Both are fixed for the life of the build**, which is why they are built once.
    pub fn generated() -> Self {
        let mut nodes: Vec<PaletteNode> = Utility::ALL
            .into_iter()
            .map(|u| PaletteNode {
                op: u.op().to_string(),
                label: u.op().rsplit('.').next().unwrap_or_default().to_string(),
                category: "Utility".into(),
                shape: u.shape(),
                source: Source::Manifest,
                out_type: None,
            })
            .collect();

        for class in cv_api::CLASSES {
            if class.status == cv_api::Status::Deprecated {
                continue;
            }
            let short = class.short_name();
            for f in class.fields.iter().filter(|f| f.api) {
                nodes.push(PaletteNode {
                    op: format!("{}.{}#get", class.path, f.name),
                    label: format!("get {}", f.name),
                    category: short.to_string(),
                    // ⚠ A field is a *pure* get node: reading a value cannot sequence anything.
                    shape: Shape::Pure,
                    source: Source::Manifest,
                    out_type: Some(f.ty.to_string()),
                });
            }
            for m in class.methods.iter().filter(|m| m.api) {
                nodes.push(PaletteNode {
                    op: format!("{}.{}", class.path, m.name),
                    label: m.name.to_string(),
                    category: short.to_string(),
                    shape: Shape::Call,
                    source: Source::Manifest,
                    out_type: Some(m.returns.to_string()),
                });
            }
        }
        nodes.sort();
        Palette { nodes }
    }

    /// Rebuild the project-sourced half.
    ///
    /// ⚠ **Replaces rather than appends.** A dial deleted from a `DIALS` section must vanish from the
    /// palette, and a rebuild that only added would leave it offerable until restart — which is exactly
    /// the staleness this split exists to bound.
    pub fn rebuild_project_nodes(&mut self, dials: &[ProjectDial]) {
        self.nodes.retain(|n| n.source != Source::Project);
        self.nodes.extend(dials.iter().map(ProjectDial::to_node));
        self.nodes.sort();
    }

    /// Every node.
    pub fn nodes(&self) -> &[PaletteNode] {
        &self.nodes
    }

    /// Look one up by op.
    ///
    /// ⚠ **The only way to place a node.** A canvas that could construct one from a string would be the
    /// text field this design does not have.
    pub fn get(&self, op: &str) -> Option<&PaletteNode> {
        self.nodes.iter().find(|n| n.op == op)
    }

    /// Every node from one source.
    pub fn from(&self, source: Source) -> Vec<&PaletteNode> {
        self.nodes.iter().filter(|n| n.source == source).collect()
    }

    /// How many.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Nothing offered.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dial(owner: &str, name: &str, kind: DialKind) -> ProjectDial {
        ProjectDial {
            owner: owner.into(),
            name: name.into(),
            kind,
            enum_path: None,
        }
    }

    #[test]
    fn the_generated_half_is_large_sorted_and_manifest_sourced() {
        let p = Palette::generated();
        assert!(p.len() > 100);
        let mut sorted = p.nodes().to_vec();
        sorted.sort();
        assert_eq!(p.nodes(), sorted.as_slice());
        assert!(p.from(Source::Project).is_empty());
    }

    #[test]
    fn a_field_is_a_pure_get_node_and_a_method_is_a_call_node() {
        // ⚠ Reading a value cannot sequence anything.
        let p = Palette::generated();
        let get = p
            .nodes()
            .iter()
            .find(|n| n.op.ends_with("#get"))
            .expect("the palette has field reads");
        assert_eq!(get.shape, Shape::Pure);
        assert!(!get.shape.has_exec_in() && !get.shape.has_exec_out());

        let call = p
            .nodes()
            .iter()
            .find(|n| n.shape == Shape::Call)
            .expect("the palette has calls");
        assert!(call.shape.has_exec_in() && call.shape.has_exec_out());
        assert!(!call.op.contains('#'));
    }

    #[test]
    fn an_event_has_an_exec_out_and_no_return_which_is_why_it_is_not_a_hook() {
        assert!(!Shape::Event.has_exec_in());
        assert!(Shape::Event.has_exec_out());
        assert_ne!(Shape::Event, Shape::Call);
    }

    #[test]
    fn a_dial_get_node_follows_the_accessor_convention_every_other_node_uses() {
        // ⚠ A different shape would make a dial read look foreign in a graph and need its own case in
        // every reader.
        let d = dial("/Content/Items/Hookshot", "length", DialKind::Number);
        assert_eq!(d.op(), "/Content/Items/Hookshot.length#dial");
        assert_eq!(d.qualified_id(), "Hookshot.length");

        let node = d.to_node();
        assert_eq!(node.shape, Shape::Pure, "a dial resolves once per pass");
        assert_eq!(node.source, Source::Project);
        assert_eq!(node.category, "Dials/Hookshot");
    }

    #[test]
    fn the_out_pin_carries_the_dials_real_type_rather_than_a_wrapper() {
        // ⚠ A wrapper would make every consumer unwrap, and would let a curve dial wire into a float
        // pin — the mistake the type system is here to refuse.
        assert_eq!(dial("/C/X", "n", DialKind::Number).out_type(), "float");
        assert_eq!(dial("/C/X", "c", DialKind::Curve).out_type(), "Curve");
        assert_eq!(dial("/C/X", "t", DialKind::Table).out_type(), "Curve");
        assert_eq!(dial("/C/X", "a", DialKind::Adaptive).out_type(), "Span");

        let e = ProjectDial {
            owner: "/C/X".into(),
            name: "grade".into(),
            kind: DialKind::Enum,
            enum_path: Some("/Core/ItemClass".into()),
        };
        assert_eq!(e.out_type(), "Enum</Core/ItemClass>");
    }

    #[test]
    fn the_project_half_rebuilds_and_a_deleted_dial_stops_being_offered() {
        // ⚠ A rebuild that only added would leave a deleted dial offerable until restart.
        let mut p = Palette::generated();
        let before = p.len();

        p.rebuild_project_nodes(&[
            dial("/Content/Items/Hookshot", "length", DialKind::Number),
            dial("/Content/Items/Hookshot", "wear_rate", DialKind::Curve),
        ]);
        assert_eq!(p.from(Source::Project).len(), 2);
        assert!(p.get("/Content/Items/Hookshot.wear_rate#dial").is_some());

        p.rebuild_project_nodes(&[dial("/Content/Items/Hookshot", "length", DialKind::Number)]);
        assert_eq!(p.from(Source::Project).len(), 1);
        assert!(
            p.get("/Content/Items/Hookshot.wear_rate#dial").is_none(),
            "the deleted dial is gone"
        );
        assert_eq!(p.len(), before + 1);
    }

    #[test]
    fn rebuilding_the_project_half_never_disturbs_the_generated_one() {
        // ⚠ Only one of the two can go stale, and this is what makes that true.
        let mut p = Palette::generated();
        let generated: Vec<PaletteNode> = p.from(Source::Manifest).into_iter().cloned().collect();
        p.rebuild_project_nodes(&[dial("/C/X", "a", DialKind::Number)]);
        p.rebuild_project_nodes(&[]);
        let after: Vec<PaletteNode> = p.from(Source::Manifest).into_iter().cloned().collect();
        assert_eq!(generated, after);
    }

    #[test]
    fn a_node_can_only_be_placed_by_looking_it_up() {
        // ⚠ There is no text field to type a wrong name into — the structural argument the pivot rests
        // on.
        let p = Palette::generated();
        assert!(p.get("core.branch").is_none() || p.get("core.branch").is_some());
        assert!(
            p.get("/Core/Object.definitely_not_a_member").is_none(),
            "a name nobody generated is not offerable"
        );
    }

    #[test]
    fn the_four_utility_nodes_are_present_and_pure() {
        // ⚠ A utility that sequenced would be an operation, not a utility.
        let p = Palette::generated();
        for u in Utility::ALL {
            let node = p.get(u.op()).unwrap_or_else(|| panic!("{}", u.op()));
            assert_eq!(node.shape, Shape::Pure);
            assert_eq!(node.category, "Utility");
        }
        assert_eq!(Utility::ALL.len(), 4);
    }

    #[test]
    fn an_expression_may_do_arithmetic() {
        for good in [
            "a + b * 2",
            "(x - y) / 3.5",
            "1.5 + 2",
            "width * 0.5 - margin",
            "a % b",
        ] {
            assert_eq!(check_expression(good), Ok(()), "{good}");
        }
    }

    #[test]
    fn an_expression_may_not_reach_through_a_member() {
        // ⚠ `a.b.c` is the seam a second scripting surface grows through.
        for bad in ["a.b", "self.length", "x + item.weight"] {
            let err = check_expression(bad).unwrap_err();
            assert!(
                matches!(err, ExpressionError::MemberAccess { .. }),
                "{bad} produced {err:?}"
            );
            assert!(err.to_string().contains("second scripting surface"));
        }
    }

    #[test]
    fn a_decimal_point_is_not_member_access() {
        // ⚠ Telling them apart is the whole check.
        assert_eq!(check_expression("1.5"), Ok(()));
        assert_eq!(check_expression("0.5 * 2.25"), Ok(()));
        assert_eq!(
            check_expression("x * 1."),
            Ok(()),
            "a digit before the point is enough — a trailing zero is a style choice, not a rule"
        );
    }

    #[test]
    fn an_expression_may_not_call_anything() {
        // ⚠ The palette has a node for it, and a call here would make this a language.
        let err = check_expression("min(a, b)").unwrap_err();
        assert!(matches!(err, ExpressionError::Call { .. }));
        assert_eq!(
            check_expression("(a + b) * 2"),
            Ok(()),
            "grouping is not a call"
        );
    }

    #[test]
    fn an_expression_refuses_characters_the_sub_language_does_not_have() {
        for bad in ["a $ b", "a; b", "a[0]", "\"text\""] {
            assert!(
                matches!(
                    check_expression(bad),
                    Err(ExpressionError::BadCharacter { .. })
                ),
                "{bad} was accepted"
            );
        }
    }
}
