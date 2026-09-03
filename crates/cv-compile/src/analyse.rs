//! **Analysis** — every check that runs between a parsed graph and a lowered one.
//!
//! ⚠ **Three tiers, and only two of them live here.** *Impossible* is the editor's — a `Kind<T>` pin
//! that will not connect to a `Ref<T>` pin is a wire that does not draw, and the payoff of a visual
//! language is that the mistake never becomes a document. What reaches this module is what a document
//! can still be wrong about: **errors**, which stop the compile, and **warnings**, which do not.
//!
//! # An override that matches no hook is an error with a hint
//!
//! ⚠ **Mostly prevented by picking from a list — and *mostly* is why the check exists.** Content can be
//! generated programmatically, and a schematic emitted by a script will happily declare `footprnt`. The
//! hint is not a nicety: without it the message is *"no such hook"* against a list of eighty, and the
//! developer greps.
//!
//! # Determinism is checked, not hoped for
//!
//! ⚠ **Three ways a graph can be non-deterministic**, and all three are cheap to detect and impossible
//! to notice by reading: iteration order escaping into a result, a float equality test, and randomness
//! from anywhere but `ctx.rng`. A generator whose output depends on any of them produces a world no seed
//! explains.

use crate::ops::Op;
use cv_cvb::parse::{Block, Line};
use cv_cvb::value::Value;
use std::fmt;

/// How much a finding stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Compilation fails.
    Error,
    /// Compilation continues.
    Warning,
    /// ⚠ **Dismissible, never enforcement.** A lint that blocked would make consistency a gate rather
    /// than a nudge, and the palette is generated from these names — so inconsistency is permanent but
    /// blocking on it is worse.
    Lint,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Lint => "lint",
        })
    }
}

/// One thing analysis found.
///
/// ⚠ **Every finding names the node and, where there is one, the pin.** *"Type mismatch"* against a
/// graph of forty nodes is a search; *"node `n_0002`, pin `value`"* is a click.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// How much it stops.
    pub severity: Severity,
    /// Which node.
    pub node: String,
    /// Which pin, when the finding is about one.
    pub pin: Option<String>,
    /// What is wrong.
    pub message: String,
    /// What to do about it, when there is a specific answer.
    pub hint: Option<String>,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: node {}", self.severity, self.node)?;
        if let Some(pin) = &self.pin {
            write!(f, ", pin {pin}")?;
        }
        write!(f, " — {}", self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, " ({hint})")?;
        }
        Ok(())
    }
}

/// What analysis produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Analysis {
    /// Everything found, in document order.
    pub findings: Vec<Finding>,
}

impl Analysis {
    /// Did anything stop the compile?
    pub fn failed(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// Only the findings of one severity.
    pub fn of(&self, severity: Severity) -> Vec<&Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .collect()
    }

    fn push(
        &mut self,
        severity: Severity,
        node: &str,
        pin: Option<&str>,
        message: impl Into<String>,
        hint: Option<String>,
    ) {
        self.findings.push(Finding {
            severity,
            node: node.to_string(),
            pin: pin.map(str::to_string),
            message: message.into(),
            hint,
        });
    }
}

/// Analyse a parsed schematic against the generated API.
pub fn analyse(root: &Block) -> Analysis {
    let mut out = Analysis::default();
    let owner = root
        .header_get("Extends")
        .and_then(|v| match v {
            Value::Reference { path, .. } => Some(path.clone()),
            _ => None,
        })
        .unwrap_or_default();

    check_overrides(root, &owner, &mut out);
    walk(root, &mut out);
    check_naming(root, &mut out);
    out
}

/// Every `Begin Graph Role=Hook` must name a hook the owner actually has.
fn check_overrides(root: &Block, owner: &str, out: &mut Analysis) {
    let hooks = hooks_of(owner);
    for graph in root.blocks("Graph") {
        let is_hook = matches!(graph.header_get("Role"), Some(Value::Ident(r)) if r == "Hook");
        if !is_hook {
            continue;
        }
        let Some(name) = graph.header_get("Name").map(text) else {
            continue;
        };
        let id = graph.header_get("Id").map(text).unwrap_or_default();
        if hooks.is_empty() {
            // Nothing known about the owner — refusing here would fail every schematic whose base
            // class lives outside the generated palette.
            continue;
        }
        if !hooks.contains(&name) {
            out.push(
                Severity::Error,
                &id,
                None,
                format!("no hook named `{name}` on {owner}"),
                nearest(&name, &hooks).map(|s| format!("did you mean `{s}`?")),
            );
        }
    }
}

fn hooks_of(owner: &str) -> Vec<String> {
    let Some(class) = cv_api::find(owner) else {
        return Vec::new();
    };
    // ⚠ **`ancestors` is strict — it walks upward and does not include the class itself.** Reading
    // it as inclusive rejected every hook a class declares directly, which is most of them: a
    // `TetherComponent` overriding `run` was told `/Core/TraversalComponent` has no such hook, by the
    // very table that declares it.
    std::iter::once(class)
        .chain(cv_api::ancestors(class))
        .flat_map(|c| c.hooks().map(|h| h.name.to_string()))
        .collect()
}

/// The closest candidate within an edit distance a typo plausibly covers.
///
/// ⚠ **Bounded, so a wrong hint is not offered.** *"Did you mean `judge`?"* against `footprnt` is worse
/// than no hint: it sends a developer to read a hook that has nothing to do with their problem.
pub fn nearest<'a>(written: &str, candidates: &'a [String]) -> Option<&'a str> {
    let budget = (written.len() / 3).max(1) + 1;
    candidates
        .iter()
        .map(|c| (distance(written, c), c.as_str()))
        .filter(|(d, _)| *d <= budget)
        .min_by_key(|(d, c)| (*d, c.len()))
        .map(|(_, c)| c)
}

/// Levenshtein, iterative, two rows.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Walk every node, checking ops, loops, determinism and dangling links.
fn walk(b: &Block, out: &mut Analysis) {
    for child in b.children() {
        if child.kind == "Node" {
            check_node(child, out);
        }
        walk(child, out);
    }
}

fn check_node(node: &Block, out: &mut Analysis) {
    let id = node.header_get("Id").map(text).unwrap_or_default();
    let Some(op) = node.header_get("Op").map(text) else {
        out.push(Severity::Error, &id, None, "a node with no Op=", None);
        return;
    };

    // A dial read is namespaced by ownership rather than by palette.
    if op.ends_with("#dial") {
        return;
    }

    // ⚠ **Determinism is checked before the op is looked up.** A `system.random` node is
    // non-deterministic whether or not the palette has it, and reporting only *"no such op"* would send
    // a developer looking for a typo when what they wrote is a category of thing that cannot exist here.
    check_determinism(node, &id, out);
    check_dangling(node, &id, out);

    let known = Op::from_name(&op);
    if known.is_none() && !is_api_call(&op) {
        let mut candidates: Vec<String> = Op::ALL.iter().map(|o| o.name().to_string()).collect();
        candidates.extend(api_ops());
        out.push(
            Severity::Error,
            &id,
            None,
            format!("no op named `{op}`"),
            nearest(&op, &candidates).map(|s| format!("did you mean `{s}`?")),
        );
        return;
    }

    if let Some(op) = known {
        // ⚠ **An unbounded loop is a compile error.** Generation must terminate, and a graph that can
        // spin is a graph that can hang the editor.
        if op == Op::For && !has_literal_bound(node) {
            out.push(
                Severity::Error,
                &id,
                Some("count"),
                "a For must carry a literal bound",
                Some("generation must terminate; there is no While".into()),
            );
        }
        if op.takes_subgraph() && node.blocks("Graph").is_empty() && node.get("Predicate").is_none()
        {
            out.push(
                Severity::Error,
                &id,
                Some("predicate"),
                format!("{op} needs a sub-graph predicate"),
                None,
            );
        }
    }
}

fn has_literal_bound(node: &Block) -> bool {
    node.pins().iter().any(|p| {
        p.get("Name").map(text) == Some("count".into())
            && matches!(p.get("Value"), Some(Value::Number { .. }))
    })
}

/// The three ways a graph escapes determinism.
fn check_determinism(node: &Block, id: &str, out: &mut Analysis) {
    let op = node.header_get("Op").map(text).unwrap_or_default();

    // ⚠ **`ctx.rng` is the only randomness.** Anything else is a value no seed explains.
    if (op.contains("random") || op.contains("shuffle")) && !op.contains("rng") {
        out.push(
            Severity::Error,
            id,
            None,
            format!("`{op}` is randomness that does not come from ctx.rng"),
            Some("a world the seed does not explain cannot be reproduced".into()),
        );
    }

    // ⚠ **Float equality.** Two paths that compute the same value by different arithmetic differ in the
    // last bit, so `==` on floats makes the result depend on the order of operations.
    for pin in node.pins() {
        let ty = pin.get("Type").map(text).unwrap_or_default();
        if ty == "float" && (op.ends_with(".eq") || op.ends_with(".equals")) {
            out.push(
                Severity::Error,
                id,
                pin.get("Name").map(text).as_deref(),
                "float equality",
                Some(
                    "compare within a tolerance; the last bit depends on the order of operations"
                        .into(),
                ),
            );
        }
    }

    // ⚠ **Iteration order escaping.** A set has no order, so a result built from one is a result the
    // next run may build differently.
    if op == "core.iterate_unordered" {
        out.push(
            Severity::Error,
            id,
            None,
            "iteration order over an unordered collection escapes into the result",
            Some("sort first, or the next run may order it differently".into()),
        );
    }
}

/// A `To=` naming a node the graph does not contain.
fn check_dangling(node: &Block, id: &str, out: &mut Analysis) {
    for pin in node.pins() {
        let Some(to) = pin.get("To") else { continue };
        let Value::Tuple(entries) = to else { continue };
        for (_, target) in entries {
            let t = text(target);
            if t.is_empty() {
                out.push(
                    Severity::Warning,
                    id,
                    pin.get("Name").map(text).as_deref(),
                    "an empty link target",
                    None,
                );
            }
        }
    }
}

/// The naming lint.
///
/// ⚠ **Non-blocking and dismissible, but present.** The palette is *generated from these names*, so an
/// inconsistency ships to every developer who ever uses the class — which is a strong reason to nudge
/// and a bad reason to block.
fn check_naming(root: &Block, out: &mut Analysis) {
    if let Some(path) = root.header_get("Path").map(text) {
        if let Some(last) = path.rsplit('/').next() {
            if !last.is_empty() && !is_pascal(last) {
                out.push(
                    Severity::Lint,
                    &root.header_get("Id").map(text).unwrap_or_default(),
                    None,
                    format!("class `{last}` is not PascalCase"),
                    None,
                );
            }
        }
    }
    lint_names(root, out);
}

fn lint_names(b: &Block, out: &mut Analysis) {
    for child in b.children() {
        if matches!(child.kind.as_str(), "Component" | "Dial" | "Graph") {
            if let Some(name) = child.header_get("Name").map(text) {
                if !name.is_empty() && !is_snake(&name) {
                    out.push(
                        Severity::Lint,
                        &child.header_get("Id").map(text).unwrap_or_default(),
                        None,
                        format!("`{name}` is not snake_case"),
                        None,
                    );
                }
            }
        }
        lint_names(child, out);
    }
}

fn is_pascal(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_uppercase()) && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn is_snake(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Is this the name of a generated API call?
fn is_api_call(op: &str) -> bool {
    api_ops().iter().any(|o| o == op)
}

/// Every op the manifest generates: `<class>.<member>`, lowercased class.
fn api_ops() -> Vec<String> {
    let mut out = Vec::new();
    for class in cv_api::CLASSES {
        let prefix = class.short_name().to_ascii_lowercase();
        for m in class.methods {
            out.push(format!("{prefix}.{}", m.name));
        }
        for f in class.fields {
            out.push(format!("{prefix}.{}", f.name));
        }
    }
    // `core.` is the context namespace, which the palette spells without a class.
    out.extend(
        ["instances_of", "state_of", "rng", "tolerance", "dial"]
            .into_iter()
            .map(|m| format!("core.{m}")),
    );
    out
}

fn text(v: &Value) -> String {
    match v {
        Value::Ident(s) | Value::Quoted(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Every line kind a block can hold, for callers that walk one.
pub fn lines_of(b: &Block) -> &[Line] {
    &b.lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use cv_cvb::parse::parse;

    fn schematic(body: &str) -> Block {
        parse(&format!(
            "Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=s\n\
             {body}End Schematic\n"
        ))
        .unwrap()
    }

    #[test]
    fn a_clean_graph_produces_no_errors() {
        let doc = schematic(
            "   Begin Graph Name=\"requires\" Role=Hook Id=grf\n      \
             Begin Node Id=n_0001 Op=core.instances_of Pos=(0,0)\n         \
             Pin (Name=out, Dir=Out, Type=bool, To=(n_0002.cond))\n      End Node\n      \
             Begin Node Id=n_0002 Op=core.branch Pos=(80,0)\n      End Node\n   End Graph\n",
        );
        let a = analyse(&doc);
        assert!(!a.failed(), "{:?}", a.of(Severity::Error));
    }

    #[test]
    fn an_override_matching_no_hook_is_an_error_with_a_fuzzy_hint() {
        // ⚠ Mostly prevented by picking from a list — and *mostly* is why the check exists.
        let doc = schematic("   Begin Graph Name=\"grnts\" Role=Hook Id=grf\n   End Graph\n");
        let a = analyse(&doc);
        assert!(a.failed());
        let e = a.of(Severity::Error)[0];
        assert!(e.message.contains("no hook named `grnts`"), "{e}");
        assert_eq!(
            e.hint.as_deref(),
            Some("did you mean `grants`?"),
            "the hint is what stops the developer grepping a list of eighty"
        );
    }

    #[test]
    fn a_real_hook_is_accepted() {
        for hook in ["grants", "requires", "judge"] {
            let doc = schematic(&format!(
                "   Begin Graph Name=\"{hook}\" Role=Hook Id=grf\n   End Graph\n"
            ));
            assert!(!analyse(&doc).failed(), "{hook} is a real hook on Item");
        }
    }

    #[test]
    fn a_wrong_hint_is_not_offered_when_nothing_is_close() {
        // ⚠ "Did you mean `judge`?" against `zzzzzzzzzz` is worse than no hint.
        let candidates: Vec<String> = ["grants", "judge", "requires"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(nearest("grnts", &candidates), Some("grants"));
        assert_eq!(nearest("zzzzzzzzzz", &candidates), None);
    }

    #[test]
    fn an_unknown_op_is_an_error_naming_the_node() {
        let doc = schematic(
            "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_0009 Op=array.is_emty Pos=(0,0)\n      End Node\n   End Graph\n",
        );
        let a = analyse(&doc);
        let e = a.of(Severity::Error)[0];
        assert_eq!(e.node, "n_0009", "the finding must name the node");
        assert!(e.message.contains("no op named"));
        assert_eq!(e.hint.as_deref(), Some("did you mean `array.is_empty`?"));
    }

    #[test]
    fn an_unbounded_for_does_not_compile() {
        // ⚠ Generation must terminate; a graph that can spin can hang the editor.
        let doc = schematic(
            "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_loop Op=core.for Pos=(0,0)\n      End Node\n   End Graph\n",
        );
        let a = analyse(&doc);
        assert!(a.failed());
        let e = a.of(Severity::Error)[0];
        assert_eq!(e.pin.as_deref(), Some("count"));
        assert!(e.hint.as_deref().unwrap().contains("no While"));
    }

    #[test]
    fn a_for_with_a_literal_bound_compiles() {
        let doc = schematic(
            "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_loop Op=core.for Pos=(0,0)\n         \
             Pin (Name=count, Dir=In, Type=int, Value=8)\n      End Node\n   End Graph\n",
        );
        assert!(!analyse(&doc).failed());
    }

    #[test]
    fn a_collection_op_without_its_subgraph_does_not_compile() {
        let doc = schematic(
            "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_f Op=array.filter Pos=(0,0)\n      End Node\n   End Graph\n",
        );
        let a = analyse(&doc);
        assert!(a.failed());
        assert_eq!(a.of(Severity::Error)[0].pin.as_deref(), Some("predicate"));
    }

    #[test]
    fn a_collection_op_with_a_nested_graph_compiles() {
        let doc = schematic(
            "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_f Op=array.filter Pos=(0,0)\n         \
             Begin Graph Name=\"pred\" Id=sub\n         End Graph\n      End Node\n   End Graph\n",
        );
        assert!(!analyse(&doc).failed());
    }

    #[test]
    fn randomness_from_anywhere_but_ctx_rng_does_not_compile() {
        let doc = schematic(
            "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_r Op=system.random Pos=(0,0)\n      End Node\n   End Graph\n",
        );
        let a = analyse(&doc);
        assert!(a.failed());
        assert!(a
            .of(Severity::Error)
            .iter()
            .any(|e| e.message.contains("ctx.rng")));
    }

    #[test]
    fn float_equality_does_not_compile() {
        // ⚠ The last bit depends on the order of operations.
        let doc = schematic(
            "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_e Op=math.eq Pos=(0,0)\n         \
             Pin (Name=a, Dir=In, Type=float)\n      End Node\n   End Graph\n",
        );
        let a = analyse(&doc);
        assert!(a
            .of(Severity::Error)
            .iter()
            .any(|e| e.message == "float equality"));
    }

    #[test]
    fn iteration_order_escaping_does_not_compile() {
        let doc = schematic(
            "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_i Op=core.iterate_unordered Pos=(0,0)\n      End Node\n   End Graph\n",
        );
        assert!(analyse(&doc).failed());
    }

    #[test]
    fn the_naming_lint_nudges_and_never_blocks() {
        // ⚠ Dismissible, but present: the palette is generated from these names.
        let doc = parse(
            "Begin Schematic Version=1 Path=/Content/Items/hookShot Extends=Kind'/Core/Item' Id=s\n   \
             Begin Dial Name=\"MaxLength\" Kind=Number Id=d\n   End Dial\nEnd Schematic\n",
        )
        .unwrap();
        let a = analyse(&doc);
        assert!(!a.failed(), "a lint must never stop a compile");
        let lints = a.of(Severity::Lint);
        assert!(lints.iter().any(|l| l.message.contains("PascalCase")));
        assert!(lints.iter().any(|l| l.message.contains("snake_case")));
    }

    #[test]
    fn a_finding_prints_the_node_and_the_pin() {
        let f = Finding {
            severity: Severity::Error,
            node: "n_0002".into(),
            pin: Some("value".into()),
            message: "type mismatch".into(),
            hint: Some("connect a bool".into()),
        };
        let s = f.to_string();
        assert!(s.contains("n_0002") && s.contains("value") && s.contains("connect a bool"));
    }

    #[test]
    fn a_dial_read_node_is_not_checked_against_the_op_table() {
        let doc = schematic(
            "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_d Op=/Content/Items/Hookshot.length#dial Pos=(0,0)\n      End Node\n   \
             End Graph\n",
        );
        assert!(!analyse(&doc).failed());
    }

    #[test]
    fn a_node_with_no_op_is_an_error_rather_than_a_silent_skip() {
        let doc = schematic(
            "   Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_x Pos=(0,0)\n      End Node\n   End Graph\n",
        );
        assert!(analyse(&doc).failed());
    }

    #[test]
    fn lines_of_exposes_the_block_body() {
        let doc = schematic("   A=1\n");
        assert!(!lines_of(&doc).is_empty());
    }
}
