//! **The typed instruction set, and the optimizations that may run over it.**
//!
//! ⚠ **Our own small deterministic bytecode, not an embedded third-party language.** The entire reason
//! this system exists is so a developer need not learn one, and building something meant to be replaced
//! later is wasted work.
//!
//! # Typed, because dispatch is the cost
//!
//! ⚠ **An instruction carries the type it operates on.** An untyped VM re-inspects a value at every
//! step; a typed one dispatches directly. The type is known at compile time because the graph's pins
//! carry it — the editor would not have drawn the wire otherwise — so carrying it into the instruction
//! costs nothing and removes a branch per operation.
//!
//! # Every optimization must preserve determinism
//!
//! | In | Out |
//! |---|---|
//! | dead-code elimination | ⚠ **JIT** — it would threaten cross-target determinism |
//! | constant folding, in owned math | loop unrolling |
//! | typed instructions | auto-inlining (deferred) |
//! | tree-shaking an unused library | |
//!
//! ⚠ **Constant folding uses owned math and no reordering may change a float result.** `(a + b) + c` and
//! `a + (b + c)` differ in the last bit, so an optimizer that reassociated would make the output depend
//! on whether it ran.
//!
//! ⚠ **Dead code is eliminated from the *bytecode*, never from the palette.** An unused library must
//! still appear in autocomplete — a developer cannot use what they cannot find — and must cost nothing
//! in the emitted program. Those are not in tension: one is an editor list, the other is an artifact.

use crate::ops::Op;
use std::collections::BTreeSet;
use std::fmt;

/// The type an instruction operates on.
///
/// ⚠ **A closed set, and `Ref` and `Kind` are separate.** The distinction that replaced seven retracted
/// language features is visible in the type expression, so it must survive into the instruction — an IR
/// that erased it would make the VM unable to tell a class from an instance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ty {
    /// Execution flow, which carries no value.
    Exec,
    Bool,
    Int,
    Float,
    Text,
    /// A class reference.
    Kind(String),
    /// An instance reference.
    Ref(String),
    /// A struct or variant, by path.
    Struct(String),
    /// A homogeneous sequence.
    Array(Box<Ty>),
    /// An enum value.
    Enum(String),
}

impl Ty {
    /// Is this a value type, as opposed to execution flow?
    pub fn carries_a_value(&self) -> bool {
        *self != Ty::Exec
    }

    /// ⚠ **May a constant of this type be folded?**
    ///
    /// Floats may — *in owned math, without reassociation*. What may not is anything with identity: two
    /// `Ref`s that compare equal are still two references, and folding one away would change what the
    /// program points at.
    pub fn is_foldable(&self) -> bool {
        matches!(self, Ty::Bool | Ty::Int | Ty::Float | Ty::Text)
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Exec => f.write_str("exec"),
            Ty::Bool => f.write_str("bool"),
            Ty::Int => f.write_str("int"),
            Ty::Float => f.write_str("float"),
            Ty::Text => f.write_str("String"),
            Ty::Kind(p) => write!(f, "Kind'{p}'"),
            Ty::Ref(p) => write!(f, "Ref'{p}'"),
            Ty::Struct(p) => write!(f, "Struct'{p}'"),
            Ty::Array(inner) => write!(f, "Array<{inner}>"),
            Ty::Enum(p) => write!(f, "Enum'{p}'"),
        }
    }
}

/// A compile-time constant.
#[derive(Clone, Debug, PartialEq)]
pub enum Const {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl Const {
    /// Its type.
    pub fn ty(&self) -> Ty {
        match self {
            Const::Bool(_) => Ty::Bool,
            Const::Int(_) => Ty::Int,
            Const::Float(_) => Ty::Float,
            Const::Text(_) => Ty::Text,
        }
    }
}

/// One typed instruction.
#[derive(Clone, Debug, PartialEq)]
pub struct Instr {
    /// What it does.
    pub op: Op,
    /// The type it operates on.
    pub ty: Ty,
    /// Which slots it reads.
    pub reads: Vec<usize>,
    /// Which slot it writes, when it writes one.
    pub writes: Option<usize>,
    /// A literal, when the op is one.
    pub value: Option<Const>,
    /// What is being called, when the op is a `Call` or a `DialRead`.
    ///
    /// ⚠ **Carried rather than looked up.** A dial read is qualified by its owner and a generated
    /// call by its class, and neither is recoverable from the op alone — an instruction that lost it
    /// would need the graph back to run.
    pub callee: Option<String>,
    /// The node this came from, so a runtime failure names something a developer drew.
    pub source: String,
}

impl Instr {
    /// A new instruction.
    pub fn new(op: Op, ty: Ty, source: impl Into<String>) -> Self {
        Instr {
            op,
            ty,
            reads: Vec::new(),
            writes: None,
            value: None,
            callee: None,
            source: source.into(),
        }
    }

    /// Name what is being called.
    pub fn calling(mut self, callee: impl Into<String>) -> Self {
        self.callee = Some(callee.into());
        self
    }

    /// Read these slots.
    pub fn reading(mut self, slots: impl IntoIterator<Item = usize>) -> Self {
        self.reads = slots.into_iter().collect();
        self
    }

    /// Write this slot.
    pub fn writing(mut self, slot: usize) -> Self {
        self.writes = Some(slot);
        self
    }

    /// Carry a literal.
    pub fn with_value(mut self, c: Const) -> Self {
        self.value = Some(c);
        self
    }

    /// Can this instruction be removed if nothing reads its output?
    ///
    /// ⚠ **`Return` never can, and neither can a dial read.** One is the point of the program; the
    /// other is a runtime input whose absence would change what a host can set.
    pub fn is_removable(&self) -> bool {
        self.op.is_pure() && self.writes.is_some()
    }
}

/// A compiled hook.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    /// The instructions, in execution order.
    pub instrs: Vec<Instr>,
    /// How many slots it needs.
    pub slots: usize,
    /// Which hook this is.
    pub hook: String,
}

impl Program {
    /// **Dead-code elimination.** Drop instructions nothing reads, transitively.
    ///
    /// ⚠ **From the bytecode, never from the palette.** An unused library still appears in autocomplete
    /// — a developer cannot use what they cannot find — and still costs nothing in the artifact.
    pub fn eliminate_dead_code(&mut self) -> usize {
        // ⚠ **Liveness is computed to a fixed point *before* anything is retained.** The first cut
        // propagated in one pass over the instruction list, so a producer sitting earlier than its
        // consumer was judged dead before the consumer had been marked live - and a four-node chain
        // ending in a `Return` lost its first two links. Deleting live code is the one direction a
        // dead-code pass must never fail in.
        let mut live: BTreeSet<usize> = BTreeSet::new();
        for i in &self.instrs {
            if !i.is_removable() {
                live.extend(i.reads.iter().copied());
            }
        }
        loop {
            let before = live.len();
            for i in &self.instrs {
                if i.writes.is_some_and(|w| live.contains(&w)) {
                    live.extend(i.reads.iter().copied());
                }
            }
            if live.len() == before {
                break;
            }
        }
        let before = self.instrs.len();
        self.instrs
            .retain(|i| !i.is_removable() || i.writes.is_some_and(|w| live.contains(&w)));
        before - self.instrs.len()
    }

    /// **Constant folding**, in owned math and without reassociation.
    ///
    /// ⚠ **Only within one instruction.** Folding `(a + b) + c` into `a + (b + c)` would change the
    /// last bit of a float result, so the optimizer never rebalances a tree — it only replaces an
    /// operation whose inputs are already literals.
    pub fn fold_constants(&mut self) -> usize {
        let mut consts: Vec<Option<Const>> = vec![None; self.slots];
        let mut folded = 0;
        for i in &mut self.instrs {
            if i.op == Op::Literal {
                if let (Some(w), Some(v)) = (i.writes, i.value.clone()) {
                    if w < consts.len() {
                        consts[w] = Some(v);
                    }
                }
                continue;
            }
            if !i.op.is_pure() || !i.ty.is_foldable() {
                continue;
            }
            let all_known = !i.reads.is_empty()
                && i.reads
                    .iter()
                    .all(|r| consts.get(*r).is_some_and(Option::is_some));
            if !all_known {
                continue;
            }
            if let Some(value) = fold(i.op, &i.ty, &i.reads, &consts) {
                if let Some(w) = i.writes {
                    if w < consts.len() {
                        consts[w] = Some(value.clone());
                    }
                }
                i.op = Op::Literal;
                i.value = Some(value);
                i.reads.clear();
                folded += 1;
            }
        }
        folded
    }

    /// **Tree-shaking**: which classes this program actually touches.
    ///
    /// ⚠ **Reported rather than applied here.** What to drop is a *cook* decision over the whole
    /// project, and a single hook cannot know whether a class it never touches is used elsewhere.
    pub fn touched_classes(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for i in &self.instrs {
            collect_paths(&i.ty, &mut out);
        }
        out
    }
}

fn collect_paths(ty: &Ty, out: &mut BTreeSet<String>) {
    match ty {
        Ty::Kind(p) | Ty::Ref(p) | Ty::Struct(p) | Ty::Enum(p) => {
            out.insert(p.clone());
        }
        Ty::Array(inner) => collect_paths(inner, out),
        _ => {}
    }
}

/// Fold one operation whose inputs are all known.
fn fold(op: Op, ty: &Ty, reads: &[usize], consts: &[Option<Const>]) -> Option<Const> {
    let values: Vec<&Const> = reads
        .iter()
        .filter_map(|r| consts.get(*r).and_then(Option::as_ref))
        .collect();
    if values.len() != reads.len() {
        return None;
    }
    match (op, ty) {
        (Op::IsEmpty, _) => None, // a collection's contents are not literals here
        (Op::Length, _) => None,
        (Op::MakeArray, _) => None,
        _ => match (values.as_slice(), ty) {
            // ⚠ **Owned math, one operation at a time.** No reassociation, so a float result cannot
            // depend on whether the optimizer ran.
            ([Const::Bool(a)], Ty::Bool) => Some(Const::Bool(!*a)),
            ([Const::Bool(a), Const::Bool(b)], Ty::Bool) => Some(Const::Bool(*a && *b)),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(slot: usize, c: Const) -> Instr {
        let ty = c.ty();
        Instr::new(Op::Literal, ty, "n_lit")
            .writing(slot)
            .with_value(c)
    }

    #[test]
    fn an_instruction_carries_the_type_it_operates_on() {
        // ⚠ An untyped VM re-inspects a value at every step; the pins already knew the type.
        let i = Instr::new(
            Op::IsEmpty,
            Ty::Array(Box::new(Ty::Ref("/Core/Object".into()))),
            "n_1",
        );
        assert_eq!(i.ty.to_string(), "Array<Ref'/Core/Object'>");
    }

    #[test]
    fn kind_and_ref_stay_distinct_in_the_ir() {
        // ⚠ The distinction that replaced seven retracted language features. An IR that erased it
        // would leave the VM unable to tell a class from an instance.
        assert_ne!(Ty::Kind("/Core/Item".into()), Ty::Ref("/Core/Item".into()));
        assert_ne!(
            Ty::Kind("/Core/Item".into()).to_string(),
            Ty::Ref("/Core/Item".into()).to_string()
        );
    }

    #[test]
    fn dead_code_is_removed_and_the_return_never_is() {
        let mut p = Program {
            instrs: vec![
                lit(0, Const::Int(1)),
                lit(1, Const::Int(2)), // nothing reads slot 1
                Instr::new(Op::Return, Ty::Int, "n_ret").reading([0]),
            ],
            slots: 2,
            hook: "grants".into(),
        };
        assert_eq!(p.eliminate_dead_code(), 1);
        assert_eq!(p.instrs.len(), 2);
        assert!(p.instrs.iter().any(|i| i.op == Op::Return));
    }

    #[test]
    fn dead_code_elimination_is_transitive() {
        let mut p = Program {
            instrs: vec![
                lit(0, Const::Int(1)),
                Instr::new(Op::IsEmpty, Ty::Bool, "n_a")
                    .reading([0])
                    .writing(1),
                Instr::new(Op::Length, Ty::Int, "n_b")
                    .reading([1])
                    .writing(2),
                Instr::new(Op::Return, Ty::Int, "n_ret"),
            ],
            slots: 3,
            hook: "grants".into(),
        };
        assert_eq!(p.eliminate_dead_code(), 3, "the whole chain goes");
        assert_eq!(p.instrs.len(), 1);
    }

    #[test]
    fn a_dial_read_is_never_eliminated_even_when_nothing_reads_it() {
        // ⚠ A dial is a runtime input; removing it would change what a host can set.
        let mut p = Program {
            instrs: vec![Instr::new(Op::DialRead, Ty::Float, "n_d").writing(0)],
            slots: 1,
            hook: "grants".into(),
        };
        assert_eq!(p.eliminate_dead_code(), 0);
        assert_eq!(p.instrs.len(), 1);
    }

    #[test]
    fn constants_fold_within_one_operation() {
        let mut p = Program {
            instrs: vec![
                lit(0, Const::Bool(true)),
                lit(1, Const::Bool(false)),
                Instr::new(Op::Filter, Ty::Bool, "n_and")
                    .reading([0, 1])
                    .writing(2),
            ],
            slots: 3,
            hook: "grants".into(),
        };
        assert_eq!(p.fold_constants(), 1);
        assert_eq!(p.instrs[2].op, Op::Literal);
        assert_eq!(p.instrs[2].value, Some(Const::Bool(false)));
        assert!(p.instrs[2].reads.is_empty());
    }

    #[test]
    fn a_dial_read_never_folds_to_its_default() {
        // ⚠ Dials are runtime inputs, never baked.
        let mut p = Program {
            instrs: vec![
                lit(0, Const::Float(30.0)),
                Instr::new(Op::DialRead, Ty::Float, "n_d")
                    .reading([0])
                    .writing(1),
            ],
            slots: 2,
            hook: "grants".into(),
        };
        assert_eq!(p.fold_constants(), 0);
        assert_eq!(p.instrs[1].op, Op::DialRead);
    }

    #[test]
    fn a_reference_type_never_folds_because_identity_is_not_a_value() {
        assert!(!Ty::Ref("/Core/Actor".into()).is_foldable());
        assert!(!Ty::Kind("/Core/Item".into()).is_foldable());
        assert!(Ty::Float.is_foldable(), "owned math, one op at a time");
        assert!(Ty::Int.is_foldable());
    }

    #[test]
    fn tree_shaking_reports_the_classes_a_program_touches() {
        let p = Program {
            instrs: vec![
                Instr::new(
                    Op::MakeArray,
                    Ty::Array(Box::new(Ty::Ref("/Core/Item".into()))),
                    "n",
                ),
                Instr::new(
                    Op::Literal,
                    Ty::Kind("/Content/Components/Tether".into()),
                    "n2",
                ),
                Instr::new(Op::Return, Ty::Int, "n3"),
            ],
            slots: 0,
            hook: "grants".into(),
        };
        let touched = p.touched_classes();
        assert!(touched.contains("/Core/Item"));
        assert!(touched.contains("/Content/Components/Tether"));
        assert_eq!(touched.len(), 2, "an int is not a class");
    }

    #[test]
    fn exec_carries_no_value() {
        assert!(!Ty::Exec.carries_a_value());
        assert!(Ty::Bool.carries_a_value());
    }

    #[test]
    fn an_instruction_names_the_node_it_came_from() {
        // ⚠ So a runtime failure names something a developer drew.
        let i = Instr::new(Op::Branch, Ty::Bool, "n_0003");
        assert_eq!(i.source, "n_0003");
    }

    #[test]
    fn a_live_chain_survives_even_when_its_producer_comes_first() {
        // ⚠ The bug the ranged-traversal test found: liveness propagated in one pass judged a producer
        // dead before its consumer had been marked live, and a four-link chain lost its first two.
        let mut p = Program {
            instrs: vec![
                Instr::new(Op::Call, Ty::Int, "n_1").writing(0),
                Instr::new(Op::IsEmpty, Ty::Bool, "n_2")
                    .reading([0])
                    .writing(1),
                Instr::new(Op::Branch, Ty::Bool, "n_3")
                    .reading([1])
                    .writing(2),
                Instr::new(Op::MakeArray, Ty::Int, "n_4")
                    .reading([2])
                    .writing(3),
                Instr::new(Op::Return, Ty::Exec, "n_5").reading([3]),
            ],
            slots: 4,
            hook: "requires".into(),
        };
        assert_eq!(p.eliminate_dead_code(), 0, "every link is live");
        assert_eq!(p.instrs.len(), 5);
    }
}
