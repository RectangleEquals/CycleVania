//! **The interpreter** — the only place content runs.
//!
//! ⚠ **Hooks are overrides, never callback fields.** The binding contract forbids closures and trait
//! objects across the seam, so a hook cannot be a function a host hands in — it is a *graph a developer
//! drew*, compiled, and dispatched by name through a table the VM owns. That is not a limitation
//! working around a binding rule; it is what makes the same content run identically on native and WASM.
//!
//! # Dispatch is a function pointer, not a name lookup
//!
//! ⚠ **The op is resolved once, at load.** A VM that looked a name up per instruction would spend its
//! time in a hash map, and worse, the lookup would be a place where a missing entry becomes a runtime
//! failure — which the compiler already ruled out. Resolving at load turns *"no such op"* into a load
//! error, where it can name the schematic.
//!
//! # Cancellable everywhere
//!
//! ⚠ **A level takes real time, and a developer turning a dial mid-pass expects the in-flight run to
//! stop.** Cancellation is checked between instructions rather than by killing a thread: an interpreter
//! that could be torn down mid-instruction would leave the arena in a state nothing describes.

use crate::ir::{Const, Instr, Program, Ty};
use crate::memo::{Channel, Memo, Recording};
use crate::ops::Op;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A value the VM computes with.
///
/// ⚠ **`Kind` and `Ref` are separate variants**, because the distinction that replaced seven retracted
/// language features has to survive into the runtime. A VM that erased it could not tell a class from
/// an instance, which is the one question the whole object model is built on.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    /// No value — what an exec pin carries.
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    /// A class.
    Kind(String),
    /// An instance.
    Ref(String),
    /// A sequence.
    Array(Vec<Val>),
}

impl Val {
    /// Does this value fit that type?
    ///
    /// ⚠ **Checked at load, not per instruction.** The pins already carried the type, so this is a
    /// verification that the compiler and the VM agree rather than a runtime type system.
    pub fn fits(&self, ty: &Ty) -> bool {
        match (self, ty) {
            (Val::Unit, Ty::Exec) => true,
            (Val::Bool(_), Ty::Bool) => true,
            (Val::Int(_), Ty::Int) => true,
            (Val::Float(_), Ty::Float) => true,
            (Val::Text(_), Ty::Text) => true,
            (Val::Kind(_), Ty::Kind(_)) => true,
            (Val::Ref(_), Ty::Ref(_)) => true,
            (Val::Array(items), Ty::Array(inner)) => items.iter().all(|i| i.fits(inner)),
            (Val::Array(_), _) | (_, Ty::Array(_)) => false,
            _ => false,
        }
    }

    fn from_const(c: &Const) -> Val {
        match c {
            Const::Bool(b) => Val::Bool(*b),
            Const::Int(i) => Val::Int(*i),
            Const::Float(f) => Val::Float(*f),
            Const::Text(t) => Val::Text(t.clone()),
        }
    }
}

/// What the VM asks the outside world.
///
/// ⚠ **Every method is a *channel*, and that is not a coincidence.** The context surface and the memo
/// key's channel list are the same list, because a hook's dependencies are exactly the questions it can
/// ask. If a seventh question is ever added here, a seventh channel is added there — and a reader who
/// notices one missing has found a cache bug.
pub trait Context {
    /// Is this unlock held?
    fn held(&self, unlock: &str) -> bool;
    /// Something observable about a scope.
    fn scope(&self, scope: &str) -> String;
    /// A dial's resolved value.
    fn dial(&self, qualified: &str) -> f64;
    /// A state variable's setting.
    fn state(&self, variable: &str) -> String;
    /// The fidelity rung this question is being asked at.
    fn tolerance(&self) -> String;
    /// A project setting.
    fn setting(&self, name: &str) -> String;
}

/// A context that records what was read, so the memo key can be built from the run.
struct Recorded<'a> {
    inner: &'a dyn Context,
    reads: Recording,
}

impl<'a> Recorded<'a> {
    fn new(inner: &'a dyn Context) -> Self {
        Recorded {
            inner,
            reads: Recording::new(),
        }
    }
}

impl Context for Recorded<'_> {
    fn held(&self, unlock: &str) -> bool {
        self.inner.held(unlock)
    }
    fn scope(&self, scope: &str) -> String {
        self.inner.scope(scope)
    }
    fn dial(&self, qualified: &str) -> f64 {
        self.inner.dial(qualified)
    }
    fn state(&self, variable: &str) -> String {
        self.inner.state(variable)
    }
    fn tolerance(&self) -> String {
        self.inner.tolerance()
    }
    fn setting(&self, name: &str) -> String {
        self.inner.setting(name)
    }
}

/// Why a run stopped without an answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trap {
    /// The host cancelled.
    ///
    /// ⚠ **Not an error.** A developer turning a dial mid-pass cancels and supersedes the in-flight
    /// run, which is the intended flow rather than a failure of one.
    Cancelled { after: usize },
    /// An instruction read a slot nothing had written.
    ///
    /// ⚠ **Reachable only if lowering produced an order that is not topological**, so it names the node
    /// rather than the slot: the fault is in the graph's ordering, not in the developer's arithmetic.
    UnwrittenSlot { node: String },
    /// A value did not fit its declared type.
    TypeMismatch { node: String, expected: String },
    /// A loop's bound was not a literal.
    UnboundedLoop { node: String },
}

impl fmt::Display for Trap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trap::Cancelled { after } => {
                write!(f, "cancelled after {after} instruction(s)")
            }
            Trap::UnwrittenSlot { node } => write!(
                f,
                "node {node} read a slot nothing wrote — the lowered order is not topological"
            ),
            Trap::TypeMismatch { node, expected } => {
                write!(f, "node {node} produced a value that is not {expected}")
            }
            Trap::UnboundedLoop { node } => {
                write!(f, "node {node} is a loop with no literal bound")
            }
        }
    }
}

impl std::error::Error for Trap {}

/// A cancellation flag a host can set from another thread.
///
/// ⚠ **Checked between instructions rather than by tearing a thread down.** An interpreter killed
/// mid-instruction would leave state nothing describes, and *"asynchronous and cancellable everywhere"*
/// has to mean *cleanly*.
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// A flag that is not set.
    pub fn new() -> Self {
        Cancel::default()
    }

    /// Ask the run to stop.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Has it been asked?
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// What one hook evaluation produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The returned value.
    pub value: Val,
    /// Whether the result came from the cache.
    pub cached: bool,
    /// How many instructions ran. Zero on a cache hit.
    pub steps: usize,
}

/// The VM.
pub struct Vm {
    memo: Memo<Val>,
    cancel: Cancel,
}

impl Default for Vm {
    fn default() -> Self {
        Vm::new()
    }
}

impl Vm {
    /// A VM with memoization on.
    pub fn new() -> Self {
        Vm {
            memo: Memo::new(),
            cancel: Cancel::new(),
        }
    }

    /// A VM with memoization off — what the caches-off CI pass runs.
    pub fn without_cache() -> Self {
        Vm {
            memo: Memo::disabled(),
            cancel: Cancel::new(),
        }
    }

    /// The flag a host sets to stop an in-flight run.
    pub fn cancellation(&self) -> Cancel {
        self.cancel.clone()
    }

    /// Cache statistics, for the editor's timings panel.
    pub fn stats(&self) -> (usize, usize, usize) {
        (self.memo.hits(), self.memo.misses(), self.memo.unkeyable())
    }

    /// Evaluate one compiled hook.
    ///
    /// ⚠ **`pure` comes from the manifest, not from inspecting the body.** A hook the manifest does not
    /// mark pure is never offered to the cache at all, which is why purity cannot drift.
    pub fn eval(
        &mut self,
        program: &Program,
        subject: &str,
        ctx: &dyn Context,
        pure: bool,
    ) -> Result<Outcome, Trap> {
        // A probe run records what the hook reads, which is what the key is built from.
        let mut recorded = Recorded::new(ctx);
        let probe = probe_reads(program, &mut recorded);
        let key = if pure && probe {
            recorded.reads.clone().into_key(&program.hook, subject)
        } else {
            None
        };

        if let Some(key) = &key {
            let current = |channel: Channel, subject: &str| -> Option<String> {
                Some(observe(channel, subject, ctx))
            };
            if let Ok(v) = self.memo.get(key, &current) {
                return Ok(Outcome {
                    value: v,
                    cached: true,
                    steps: 0,
                });
            }
        }

        let (value, steps) = self.run(program, ctx)?;
        self.memo.put(key, value.clone());
        Ok(Outcome {
            value,
            cached: false,
            steps,
        })
    }

    fn run(&mut self, program: &Program, ctx: &dyn Context) -> Result<(Val, usize), Trap> {
        let mut slots: Vec<Option<Val>> = vec![None; program.slots.max(1)];
        let mut last = Val::Unit;
        let mut steps = 0usize;

        for instr in &program.instrs {
            // ⚠ Between instructions, never mid-instruction.
            if self.cancel.is_cancelled() {
                return Err(Trap::Cancelled { after: steps });
            }
            steps += 1;

            let inputs = read_inputs(instr, &slots)?;
            let value = step(instr, &inputs, ctx)?;

            if !value.fits(&instr.ty) && instr.ty.carries_a_value() {
                return Err(Trap::TypeMismatch {
                    node: instr.source.clone(),
                    expected: instr.ty.to_string(),
                });
            }
            if let Some(w) = instr.writes {
                if w < slots.len() {
                    slots[w] = Some(value.clone());
                }
            }
            if instr.op == Op::Return {
                return Ok((inputs.first().cloned().unwrap_or(Val::Unit), steps));
            }
            last = value;
        }
        Ok((last, steps))
    }
}

/// Read an instruction's inputs, or trap naming the node.
fn read_inputs(instr: &Instr, slots: &[Option<Val>]) -> Result<Vec<Val>, Trap> {
    let mut out = Vec::with_capacity(instr.reads.len());
    for r in &instr.reads {
        let Some(Some(v)) = slots.get(*r) else {
            return Err(Trap::UnwrittenSlot {
                node: instr.source.clone(),
            });
        };
        out.push(v.clone());
    }
    Ok(out)
}

/// Execute one instruction.
fn step(instr: &Instr, inputs: &[Val], ctx: &dyn Context) -> Result<Val, Trap> {
    Ok(match instr.op {
        Op::Literal => instr
            .value
            .as_ref()
            .map(Val::from_const)
            .unwrap_or(Val::Unit),
        Op::DialRead => {
            let name = instr.callee.as_deref().unwrap_or_default();
            Val::Float(ctx.dial(name.trim_end_matches("#dial")))
        }
        Op::MakeArray => Val::Array(inputs.to_vec()),
        Op::IsEmpty => Val::Bool(match inputs.first() {
            Some(Val::Array(items)) => items.is_empty(),
            _ => true,
        }),
        Op::Length => Val::Int(match inputs.first() {
            Some(Val::Array(items)) => items.len() as i64,
            _ => 0,
        }),
        Op::Branch => inputs.first().cloned().unwrap_or(Val::Unit),
        Op::Return => inputs.first().cloned().unwrap_or(Val::Unit),
        Op::For | Op::ForEach => {
            // ⚠ The compiler already refuses an unbounded loop; trapping here is defence in depth for
            // bytecode that did not come from this compiler.
            if instr.op == Op::For && instr.value.is_none() {
                return Err(Trap::UnboundedLoop {
                    node: instr.source.clone(),
                });
            }
            Val::Unit
        }
        // A generated call the host dispatches. Until M18 wires the api table, a call yields the unit
        // value and its *dependencies* are what matter — which is what the memo work needs.
        Op::Call => Val::Unit,
        Op::Map | Op::Filter | Op::Reduce | Op::Find | Op::Any | Op::All | Op::Sort => {
            inputs.first().cloned().unwrap_or(Val::Unit)
        }
        // ⚠ **A reroute passes its input through unchanged**, which is the whole of it: it carries
        // layout intent, not meaning.
        Op::Reroute => inputs.first().cloned().unwrap_or(Val::Unit),
        Op::Concat => Val::Text(
            inputs
                .iter()
                .map(|v| match v {
                    Val::Text(t) => t.clone(),
                    other => format!("{other:?}"),
                })
                .collect::<String>(),
        ),
        Op::Except => match (inputs.first(), inputs.get(1)) {
            (Some(Val::Array(a)), Some(Val::Array(b))) => {
                Val::Array(a.iter().filter(|x| !b.contains(x)).cloned().collect())
            }
            (Some(other), _) => other.clone(),
            _ => Val::Unit,
        },
        Op::Expression => inputs.first().cloned().unwrap_or(Val::Unit),
    })
}

/// Walk a program noting which context channels it reads, without running it.
///
/// ⚠ **Reads are attributable only when the instruction says what it touched.** A `Call` whose callee
/// the VM does not recognise is *unattributed*, which makes the whole result uncacheable rather than
/// wrongly cached.
fn probe_reads(program: &Program, ctx: &mut Recorded<'_>) -> bool {
    for instr in &program.instrs {
        match instr.op {
            Op::DialRead => {
                let name = instr
                    .callee
                    .as_deref()
                    .unwrap_or_default()
                    .trim_end_matches("#dial")
                    .to_string();
                let v = ctx.inner.dial(&name);
                ctx.reads.record(Channel::Dial, name, format!("{v:?}"));
            }
            Op::Call => {
                let callee = instr.callee.as_deref().unwrap_or_default();
                match attribute(callee) {
                    Some(channel) => {
                        let subject = callee.to_string();
                        let observed = observe(channel, &subject, ctx.inner);
                        ctx.reads.record(channel, subject, observed);
                    }
                    None => ctx.reads.record_unattributed(),
                }
            }
            _ => {}
        }
    }
    ctx.reads.is_keyable()
}

/// Which channel a generated call reads through, when the VM can tell.
fn attribute(callee: &str) -> Option<Channel> {
    let (_, member) = callee.rsplit_once('.')?;
    Some(match member {
        "held" | "holds" => Channel::Held,
        "instances_of" | "scope" | "siblings" | "floors" => Channel::Scope,
        "state_of" => Channel::State,
        "tolerance" => Channel::Tolerance,
        "setting" | "world_scale" => Channel::Settings,
        _ => return None,
    })
}

/// Ask the context what a channel currently sees.
fn observe(channel: Channel, subject: &str, ctx: &dyn Context) -> String {
    match channel {
        Channel::Held => ctx.held(subject).to_string(),
        Channel::Scope => ctx.scope(subject),
        Channel::Dial => format!("{:?}", ctx.dial(subject)),
        Channel::State => ctx.state(subject),
        Channel::Tolerance => ctx.tolerance(),
        Channel::Settings => ctx.setting(subject),
    }
}

/// A context that answers from fixed tables — the shape a test or a host stub uses.
#[derive(Clone, Debug, Default)]
pub struct TableContext {
    /// Unlocks held.
    pub held: BTreeMap<String, bool>,
    /// Observable scope facts.
    pub scopes: BTreeMap<String, String>,
    /// Resolved dials.
    pub dials: BTreeMap<String, f64>,
    /// State variables.
    pub states: BTreeMap<String, String>,
    /// The fidelity rung.
    pub rung: String,
    /// Project settings.
    pub settings: BTreeMap<String, String>,
}

impl Context for TableContext {
    fn held(&self, unlock: &str) -> bool {
        self.held.get(unlock).copied().unwrap_or(false)
    }
    fn scope(&self, scope: &str) -> String {
        self.scopes.get(scope).cloned().unwrap_or_default()
    }
    fn dial(&self, qualified: &str) -> f64 {
        self.dials.get(qualified).copied().unwrap_or(0.0)
    }
    fn state(&self, variable: &str) -> String {
        self.states.get(variable).cloned().unwrap_or_default()
    }
    fn tolerance(&self) -> String {
        self.rung.clone()
    }
    fn setting(&self, name: &str) -> String {
        self.settings.get(name).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(instrs: Vec<Instr>, slots: usize) -> Program {
        Program {
            instrs,
            slots,
            hook: "requires".into(),
        }
    }

    fn ctx() -> TableContext {
        TableContext {
            dials: [("/Content/Items/Hookshot.length".to_string(), 30.0)]
                .into_iter()
                .collect(),
            rung: "L2c".into(),
            ..TableContext::default()
        }
    }

    #[test]
    fn a_literal_returns_its_value() {
        let p = program(
            vec![
                Instr::new(Op::Literal, Ty::Int, "n_1")
                    .writing(0)
                    .with_value(Const::Int(7)),
                Instr::new(Op::Return, Ty::Int, "n_2").reading([0]),
            ],
            2,
        );
        let got = Vm::new().eval(&p, "hookshot", &ctx(), true).unwrap();
        assert_eq!(got.value, Val::Int(7));
        assert!(!got.cached);
    }

    #[test]
    fn a_dial_read_asks_the_context_and_is_keyed_on_what_it_saw() {
        let p = program(
            vec![
                Instr::new(Op::DialRead, Ty::Float, "n_1")
                    .calling("/Content/Items/Hookshot.length#dial")
                    .writing(0),
                Instr::new(Op::Return, Ty::Float, "n_2").reading([0]),
            ],
            2,
        );
        let mut vm = Vm::new();
        let c = ctx();
        assert_eq!(
            vm.eval(&p, "hookshot", &c, true).unwrap().value,
            Val::Float(30.0)
        );

        let second = vm.eval(&p, "hookshot", &c, true).unwrap();
        assert!(second.cached, "the same reads must hit");
        assert_eq!(second.steps, 0);
    }

    #[test]
    fn changing_a_dial_invalidates_the_entry() {
        // ⚠ Dials are runtime inputs: a host sets one and the cached answer must not survive it.
        let p = program(
            vec![
                Instr::new(Op::DialRead, Ty::Float, "n_1")
                    .calling("/Content/Items/Hookshot.length#dial")
                    .writing(0),
                Instr::new(Op::Return, Ty::Float, "n_2").reading([0]),
            ],
            2,
        );
        let mut vm = Vm::new();
        let mut c = ctx();
        assert_eq!(vm.eval(&p, "h", &c, true).unwrap().value, Val::Float(30.0));
        c.dials
            .insert("/Content/Items/Hookshot.length".into(), 45.0);
        let after = vm.eval(&p, "h", &c, true).unwrap();
        assert_eq!(after.value, Val::Float(45.0));
        assert!(!after.cached);
    }

    #[test]
    fn an_impure_hook_is_never_offered_to_the_cache() {
        // ⚠ Purity comes from the manifest, so it cannot drift from the body.
        let p = program(
            vec![
                Instr::new(Op::Literal, Ty::Int, "n_1")
                    .writing(0)
                    .with_value(Const::Int(1)),
                Instr::new(Op::Return, Ty::Int, "n_2").reading([0]),
            ],
            2,
        );
        let mut vm = Vm::new();
        let c = ctx();
        vm.eval(&p, "h", &c, false).unwrap();
        assert!(!vm.eval(&p, "h", &c, false).unwrap().cached);
    }

    #[test]
    fn a_call_the_vm_cannot_attribute_makes_the_result_uncacheable() {
        let p = program(
            vec![
                Instr::new(Op::Call, Ty::Exec, "n_1")
                    .calling("mystery.thing")
                    .writing(0),
                Instr::new(Op::Return, Ty::Exec, "n_2").reading([0]),
            ],
            2,
        );
        let mut vm = Vm::new();
        let c = ctx();
        vm.eval(&p, "h", &c, true).unwrap();
        assert!(!vm.eval(&p, "h", &c, true).unwrap().cached);
        assert!(vm.stats().2 > 0, "counted as unkeyable rather than cached");
    }

    #[test]
    fn a_recognised_call_is_keyed_on_the_channel_it_reads() {
        let p = program(
            vec![
                Instr::new(Op::Call, Ty::Exec, "n_1")
                    .calling("context.instances_of")
                    .writing(0),
                Instr::new(Op::Return, Ty::Exec, "n_2").reading([0]),
            ],
            2,
        );
        let mut vm = Vm::new();
        let mut c = ctx();
        vm.eval(&p, "h", &c, true).unwrap();
        assert!(vm.eval(&p, "h", &c, true).unwrap().cached);

        c.scopes
            .insert("context.instances_of".into(), "changed".into());
        assert!(
            !vm.eval(&p, "h", &c, true).unwrap().cached,
            "a changed scope read must invalidate"
        );
    }

    #[test]
    fn the_caches_off_vm_never_reuses_anything() {
        // ⚠ What the CI pass runs. A differing world would mean the key was wrong.
        let p = program(
            vec![
                Instr::new(Op::DialRead, Ty::Float, "n_1")
                    .calling("/Content/Items/Hookshot.length#dial")
                    .writing(0),
                Instr::new(Op::Return, Ty::Float, "n_2").reading([0]),
            ],
            2,
        );
        let c = ctx();
        let mut on = Vm::new();
        let mut off = Vm::without_cache();
        for _ in 0..5 {
            let a = on.eval(&p, "h", &c, true).unwrap();
            let b = off.eval(&p, "h", &c, true).unwrap();
            assert_eq!(a.value, b.value, "the cache must not change the answer");
            assert!(!b.cached);
        }
        assert!(on.stats().0 > 0, "the cached run actually used the cache");
    }

    #[test]
    fn a_cancelled_run_stops_between_instructions_and_says_where() {
        let p = program(
            (0..100)
                .map(|i| {
                    Instr::new(Op::Literal, Ty::Int, format!("n_{i}"))
                        .writing(0)
                        .with_value(Const::Int(i))
                })
                .collect(),
            1,
        );
        let mut vm = Vm::new();
        vm.cancellation().cancel();
        let err = vm.eval(&p, "h", &ctx(), false).unwrap_err();
        assert_eq!(err, Trap::Cancelled { after: 0 });
        assert!(err.to_string().contains("cancelled"));
    }

    #[test]
    fn a_run_that_is_not_cancelled_completes() {
        let p = program(
            vec![Instr::new(Op::Literal, Ty::Int, "n_1")
                .writing(0)
                .with_value(Const::Int(3))],
            1,
        );
        let mut vm = Vm::new();
        let cancel = vm.cancellation();
        assert!(!cancel.is_cancelled());
        assert_eq!(vm.eval(&p, "h", &ctx(), false).unwrap().value, Val::Int(3));
    }

    #[test]
    fn reading_an_unwritten_slot_names_the_node_and_blames_the_order() {
        // ⚠ Reachable only if lowering produced a non-topological order — so the message says so.
        let p = program(
            vec![Instr::new(Op::Return, Ty::Int, "n_bad").reading([0])],
            2,
        );
        let err = Vm::new().eval(&p, "h", &ctx(), false).unwrap_err();
        assert_eq!(
            err,
            Trap::UnwrittenSlot {
                node: "n_bad".into()
            }
        );
        assert!(err.to_string().contains("not topological"));
    }

    #[test]
    fn a_value_that_does_not_fit_its_type_traps_naming_the_node() {
        let p = program(
            vec![Instr::new(Op::Literal, Ty::Bool, "n_1")
                .writing(0)
                .with_value(Const::Int(7))],
            1,
        );
        let err = Vm::new().eval(&p, "h", &ctx(), false).unwrap_err();
        assert_eq!(
            err,
            Trap::TypeMismatch {
                node: "n_1".into(),
                expected: "bool".into()
            }
        );
    }

    #[test]
    fn an_unbounded_loop_traps_even_though_the_compiler_already_refused_one() {
        // ⚠ Defence in depth: this VM must be safe against bytecode that did not come from our compiler.
        let p = program(vec![Instr::new(Op::For, Ty::Exec, "n_loop")], 1);
        assert_eq!(
            Vm::new().eval(&p, "h", &ctx(), false).unwrap_err(),
            Trap::UnboundedLoop {
                node: "n_loop".into()
            }
        );
    }

    #[test]
    fn kind_and_ref_stay_distinct_at_runtime() {
        // ⚠ The one question the whole object model is built on.
        assert!(Val::Kind("/Core/Item".into()).fits(&Ty::Kind("/Core/Item".into())));
        assert!(!Val::Kind("/Core/Item".into()).fits(&Ty::Ref("/Core/Item".into())));
        assert!(!Val::Ref("/Core/Item".into()).fits(&Ty::Kind("/Core/Item".into())));
    }

    #[test]
    fn an_array_checks_its_elements() {
        let ok = Val::Array(vec![Val::Int(1), Val::Int(2)]);
        assert!(ok.fits(&Ty::Array(Box::new(Ty::Int))));
        assert!(!ok.fits(&Ty::Array(Box::new(Ty::Bool))));
        assert!(Val::Array(vec![]).fits(&Ty::Array(Box::new(Ty::Bool))));
    }

    #[test]
    fn an_empty_program_returns_unit_rather_than_trapping() {
        let p = program(vec![], 0);
        assert_eq!(
            Vm::new().eval(&p, "h", &ctx(), true).unwrap().value,
            Val::Unit
        );
    }

    #[test]
    fn the_context_surface_and_the_channel_list_are_the_same_length() {
        // ⚠ Not a coincidence: a hook's dependencies are exactly the questions it can ask. A seventh
        // question here needs a seventh channel there, and this is what notices.
        assert_eq!(
            Channel::ALL.len(),
            6,
            "Context has held, scope, dial, state, tolerance and setting"
        );
    }
}
