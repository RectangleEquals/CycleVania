//! **Lowering** — a graph to a typed program.
//!
//! ⚠ **A graph is already close to IR**, which is most of why this stage is short. Nodes are
//! instructions, pins are slots, and `To=` links are the data dependencies an optimizer needs. What
//! lowering actually does is pick an **order**.
//!
//! # The order is a topological sort, and ties break by node id
//!
//! ⚠ **Two orders that both satisfy the dependencies are not equally good.** One of them is the same on
//! every machine. Ties break by id — which is content-derived — so the emitted program is a property of
//! the graph rather than of the hash order a set happened to have.

use crate::ir::{Const, Instr, Program, Ty};
use crate::ops::Op;
use cv_cvb::parse::Block;
use cv_cvb::value::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Why a graph could not be lowered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LowerError {
    /// A link names a node the graph does not contain.
    DanglingLink { from: String, to: String },
    /// The data dependencies form a cycle.
    ///
    /// ⚠ **A cycle in a *pure* subgraph is an error, not a loop.** There is no order in which every
    /// instruction's inputs are ready, so the program has no meaning — which is different from a
    /// deliberate iteration, and the message has to say which one the developer wrote.
    Cycle { nodes: Vec<String> },
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LowerError::DanglingLink { from, to } => {
                write!(f, "node {from} links to {to}, which is not in this graph")
            }
            LowerError::Cycle { nodes } => write!(
                f,
                "a cycle in a pure subgraph: {} — there is no order in which every input is ready, \
                 which is not the same as a loop",
                nodes.join(" → ")
            ),
        }
    }
}

impl std::error::Error for LowerError {}

/// Lower one `Begin Graph` into a program.
pub fn lower(graph: &Block) -> Result<Program, LowerError> {
    let hook = graph
        .header_get("Name")
        .map(text)
        .unwrap_or_else(|| "anonymous".into());

    let nodes: Vec<&Block> = graph.blocks("Node");
    let ids: Vec<String> = nodes
        .iter()
        .map(|n| n.header_get("Id").map(text).unwrap_or_default())
        .collect();
    let index: BTreeMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    // Edges: a `To=(target.pin)` makes this node a dependency of `target`.
    let mut deps: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); nodes.len()];
    for (i, node) in nodes.iter().enumerate() {
        for pin in node.pins() {
            for target in link_targets(pin.get("To")) {
                let node_name = target.split('.').next().unwrap_or(&target).to_string();
                let Some(&j) = index.get(node_name.as_str()) else {
                    return Err(LowerError::DanglingLink {
                        from: ids[i].clone(),
                        to: node_name,
                    });
                };
                deps[j].insert(i);
            }
        }
    }

    let order = topological(&deps, &ids)?;

    // One slot per node output, so a slot index is a node index.
    let mut instrs = Vec::with_capacity(order.len());
    for i in order {
        let node = nodes[i];
        // ⚠ **An unrecognised op becomes a `Call`, never a literal.** Falling back to `Literal`
        // made every generated API call look like a constant, so dead-code elimination deleted whole
        // chains and constant folding was free to replace them — a silent default of exactly the kind
        // this project spends its checks preventing.
        let name = node.header_get("Op").map(text).unwrap_or_default();
        let op = if name.ends_with("#dial") {
            Op::DialRead
        } else {
            Op::from_name(&name).unwrap_or(Op::Call)
        };
        let ty = output_type(node);
        let mut instr = Instr::new(op, ty, ids[i].clone())
            .reading(deps[i].iter().copied())
            .writing(i);
        if matches!(op, Op::Call | Op::DialRead) {
            instr = instr.calling(name);
        }
        if let Some(c) = literal_of(node) {
            instr = instr.with_value(c);
        }
        instrs.push(instr);
    }

    Ok(Program {
        instrs,
        slots: nodes.len(),
        hook,
    })
}

/// Kahn's algorithm, with ties broken by node id.
///
/// ⚠ **The tie-break is the determinism.** Any topological order satisfies the dependencies; exactly
/// one of them is the same on every machine, and ids are content-derived so that one is a property of
/// the graph.
fn topological(deps: &[BTreeSet<usize>], ids: &[String]) -> Result<Vec<usize>, LowerError> {
    let n = deps.len();
    let mut remaining: Vec<BTreeSet<usize>> = deps.to_vec();
    let mut done: BTreeSet<usize> = BTreeSet::new();
    let mut out = Vec::with_capacity(n);

    while out.len() < n {
        let mut ready: Vec<usize> = (0..n)
            .filter(|i| !done.contains(i) && remaining[*i].is_empty())
            .collect();
        if ready.is_empty() {
            let stuck: Vec<String> = (0..n)
                .filter(|i| !done.contains(i))
                .map(|i| ids[i].clone())
                .collect();
            return Err(LowerError::Cycle { nodes: stuck });
        }
        ready.sort_by(|a, b| ids[*a].cmp(&ids[*b]));
        let next = ready[0];
        done.insert(next);
        out.push(next);
        for set in remaining.iter_mut() {
            set.remove(&next);
        }
    }
    Ok(out)
}

fn link_targets(v: Option<&Value>) -> Vec<String> {
    let Some(Value::Tuple(entries)) = v else {
        return Vec::new();
    };
    entries.iter().map(|(_, t)| text(t)).collect()
}

/// The type of a node's `Dir=Out` data pin.
fn output_type(node: &Block) -> Ty {
    for pin in node.pins() {
        if pin.get("Dir").map(text).as_deref() != Some("Out") {
            continue;
        }
        if let Some(t) = pin.get("Type") {
            let ty = type_of(t);
            if ty.carries_a_value() {
                return ty;
            }
        }
    }
    Ty::Exec
}

/// Map a parsed type expression onto the instruction set's types.
pub fn type_of(v: &Value) -> Ty {
    use cv_cvb::value::RefTag;
    match v {
        Value::Ident(name) => match name.as_str() {
            "exec" => Ty::Exec,
            "bool" => Ty::Bool,
            "int" => Ty::Int,
            "float" => Ty::Float,
            "String" => Ty::Text,
            other => Ty::Struct(other.to_string()),
        },
        Value::Reference { tag, path } => match tag {
            RefTag::Kind => Ty::Kind(path.clone()),
            RefTag::Ref => Ty::Ref(path.clone()),
            RefTag::Enum => Ty::Enum(path.clone()),
            _ => Ty::Struct(path.clone()),
        },
        Value::Container { name, args } if name == "Array" => {
            Ty::Array(Box::new(args.first().map(type_of).unwrap_or(Ty::Exec)))
        }
        Value::Container { name, .. } => Ty::Struct(name.clone()),
        Value::Member { base, .. } => type_of(base),
        other => Ty::Struct(other.to_string()),
    }
}

fn literal_of(node: &Block) -> Option<Const> {
    for pin in node.pins() {
        if let Some(v) = pin.get("Value") {
            return match v {
                Value::Number {
                    value,
                    fractional: false,
                } => Some(Const::Int(*value as i64)),
                Value::Number {
                    value,
                    fractional: true,
                } => Some(Const::Float(*value)),
                Value::Quoted(s) => Some(Const::Text(s.clone())),
                Value::Ident(s) if s == "true" => Some(Const::Bool(true)),
                Value::Ident(s) if s == "false" => Some(Const::Bool(false)),
                _ => None,
            };
        }
    }
    None
}

fn text(v: &Value) -> String {
    match v {
        Value::Ident(s) | Value::Quoted(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cv_cvb::parse::parse;

    fn graph(body: &str) -> Block {
        parse(&format!(
            "Begin Graph Name=\"requires\" Role=Hook Id=grf\n{body}End Graph\n"
        ))
        .unwrap()
    }

    #[test]
    fn a_chain_lowers_in_dependency_order() {
        let g = graph(
            "   Begin Node Id=n_0002 Op=array.is_empty Pos=(0,0)\n      \
             Pin (Name=out, Dir=Out, Type=bool, To=(n_0003.cond))\n   End Node\n   \
             Begin Node Id=n_0003 Op=core.branch Pos=(80,0)\n      \
             Pin (Name=cond, Dir=In, Type=bool)\n   End Node\n",
        );
        let p = lower(&g).unwrap();
        assert_eq!(p.hook, "requires");
        assert_eq!(p.instrs.len(), 2);
        assert_eq!(p.instrs[0].source, "n_0002", "the producer comes first");
        assert_eq!(p.instrs[1].source, "n_0003");
        assert!(p.instrs[1].reads.contains(&0));
    }

    #[test]
    fn the_order_is_the_same_every_time_because_ties_break_by_id() {
        // ⚠ Any topological order satisfies the dependencies; exactly one is the same on every machine.
        let g = graph(
            "   Begin Node Id=n_b Op=array.length Pos=(0,0)\n   End Node\n   \
             Begin Node Id=n_a Op=array.length Pos=(0,0)\n   End Node\n   \
             Begin Node Id=n_c Op=array.length Pos=(0,0)\n   End Node\n",
        );
        let first = lower(&g).unwrap();
        let again = lower(&g).unwrap();
        assert_eq!(first, again);
        let order: Vec<&str> = first.instrs.iter().map(|i| i.source.as_str()).collect();
        assert_eq!(order, vec!["n_a", "n_b", "n_c"]);
    }

    #[test]
    fn a_dangling_link_names_both_ends() {
        let g = graph(
            "   Begin Node Id=n_0001 Op=array.length Pos=(0,0)\n      \
             Pin (Name=out, Dir=Out, Type=int, To=(n_nowhere.value))\n   End Node\n",
        );
        assert_eq!(
            lower(&g),
            Err(LowerError::DanglingLink {
                from: "n_0001".into(),
                to: "n_nowhere".into()
            })
        );
    }

    #[test]
    fn a_cycle_in_a_pure_subgraph_is_an_error_that_says_it_is_not_a_loop() {
        let g = graph(
            "   Begin Node Id=n_a Op=array.length Pos=(0,0)\n      \
             Pin (Name=out, Dir=Out, Type=int, To=(n_b.value))\n   End Node\n   \
             Begin Node Id=n_b Op=array.length Pos=(0,0)\n      \
             Pin (Name=out, Dir=Out, Type=int, To=(n_a.value))\n   End Node\n",
        );
        let err = lower(&g).unwrap_err();
        assert!(matches!(err, LowerError::Cycle { .. }));
        let text = err.to_string();
        assert!(text.contains("n_a") && text.contains("n_b"));
        assert!(
            text.contains("not the same as a loop"),
            "the message must distinguish it from deliberate iteration: {text}"
        );
    }

    #[test]
    fn a_nodes_output_type_becomes_the_instruction_type() {
        let g = graph(
            "   Begin Node Id=n Op=core.instances_of Pos=(0,0)\n      \
             Pin (Name=kind, Dir=In, Type=Kind'/Core/Component')\n      \
             Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/Object'>)\n   End Node\n",
        );
        let p = lower(&g).unwrap();
        assert_eq!(
            p.instrs[0].ty,
            Ty::Array(Box::new(Ty::Ref("/Core/Object".into())))
        );
    }

    #[test]
    fn an_exec_only_node_has_the_exec_type() {
        let g = graph(
            "   Begin Node Id=n Op=core.branch Pos=(0,0)\n      \
             Pin (Name=true, Dir=Out, Type=exec)\n   End Node\n",
        );
        assert_eq!(lower(&g).unwrap().instrs[0].ty, Ty::Exec);
    }

    #[test]
    fn a_pin_literal_becomes_the_instructions_constant() {
        let g = graph(
            "   Begin Node Id=n Op=core.for Pos=(0,0)\n      \
             Pin (Name=count, Dir=In, Type=int, Value=8)\n   End Node\n",
        );
        assert_eq!(lower(&g).unwrap().instrs[0].value, Some(Const::Int(8)));
    }

    #[test]
    fn every_type_expression_maps_onto_the_instruction_set() {
        use cv_cvb::parse::value as parse_value;
        let cases = [
            ("exec", Ty::Exec),
            ("bool", Ty::Bool),
            ("int", Ty::Int),
            ("float", Ty::Float),
            ("String", Ty::Text),
            ("Kind'/Core/Item'", Ty::Kind("/Core/Item".into())),
            ("Ref'/Core/Actor'", Ty::Ref("/Core/Actor".into())),
            ("Enum'/Core/ItemClass'", Ty::Enum("/Core/ItemClass".into())),
        ];
        for (src, expect) in cases {
            assert_eq!(type_of(&parse_value(src, 1).unwrap()), expect, "{src}");
        }
    }

    #[test]
    fn an_empty_graph_lowers_to_an_empty_program() {
        let p = lower(&graph("")).unwrap();
        assert!(p.instrs.is_empty());
        assert_eq!(p.slots, 0);
    }

    #[test]
    fn an_unrecognised_op_lowers_to_a_call_and_never_to_a_literal() {
        // ⚠ Falling back to `Literal` made every generated API call look like a constant, so dead-code
        // elimination deleted whole chains and folding was free to replace them.
        let g = graph(
            "   Begin Node Id=n Op=core.instances_of Pos=(0,0)\n      \
             Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/Object'>)\n   End Node\n",
        );
        let p = lower(&g).unwrap();
        assert_eq!(p.instrs[0].op, Op::Call);
        assert_eq!(p.instrs[0].callee.as_deref(), Some("core.instances_of"));
    }

    #[test]
    fn a_dial_node_lowers_to_a_dial_read_that_names_its_owner() {
        let g = graph(
            "   Begin Node Id=n Op=/Content/Items/Hookshot.length#dial Pos=(0,0)\n      \
             Pin (Name=out, Dir=Out, Type=float)\n   End Node\n",
        );
        let p = lower(&g).unwrap();
        assert_eq!(p.instrs[0].op, Op::DialRead);
        assert_eq!(
            p.instrs[0].callee.as_deref(),
            Some("/Content/Items/Hookshot.length#dial"),
            "the owner is not recoverable from the op alone"
        );
    }
}
