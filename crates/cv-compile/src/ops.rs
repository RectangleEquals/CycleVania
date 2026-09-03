//! **The op table** — what a schematic node may be, beyond an API call.
//!
//! ⚠ **An API call is *most* of the palette, and it is generated.** `core.instances_of`,
//! `core.branch`, every field read and every method — those come from the manifest and need no table
//! here. What this module holds is the part that **cannot** be generated: control flow, collections and
//! literals, whose shapes are properties of the language rather than of the API surface.
//!
//! # Collections are functional, and each takes a sub-graph
//!
//! ⚠ **A predicate is a *sub-graph*, not a closure**, because a visual language has no syntax for one
//! and inventing one would be inventing a text language inside a graph. `Filter` names a graph the
//! compiler inlines per element; the developer draws it in the same editor as everything else.
//!
//! ⚠ **About 80% of real uses are a `Make Array` return**, and the remainder must not require a loop.
//! That is why `Map` · `Filter` · `Reduce` · `Find` · `Any` · `All` · `Sort` are all here: a developer
//! who reaches for a `For` to filter a list has been failed by the palette, not by their own thinking.
//!
//! # Loops are bounded, and an unbounded one does not compile
//!
//! ⚠ **Generation must terminate.** A graph that can spin is a graph that can hang the editor, so a
//! `For` carries a literal bound and a `ForEach` is bounded by the collection it walks. There is no
//! `While`, and its absence is the feature.

use std::fmt;

/// What a node does, for the ops the manifest cannot describe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op {
    // --- control flow ---
    /// Two exec outs on a bool.
    Branch,
    /// Return from the hook.
    Return,
    /// A numeric loop with a literal bound.
    For,
    /// A loop over a collection, bounded by its length.
    ForEach,
    // --- collections ---
    /// Build an array from its input pins. ⚠ The 80% case.
    MakeArray,
    /// One sub-graph per element, collecting results.
    Map,
    /// Keep the elements a sub-graph accepts.
    Filter,
    /// Fold with a sub-graph and a seed.
    Reduce,
    /// The first element a sub-graph accepts.
    Find,
    /// Does any element satisfy the sub-graph?
    Any,
    /// Do all of them?
    All,
    /// Order by a sub-graph's key.
    Sort,
    /// Is the collection empty?
    IsEmpty,
    /// How many elements.
    Length,
    // --- values ---
    /// A literal.
    Literal,
    /// Read a declared dial.
    DialRead,
    /// A generated API call — everything the manifest describes.
    ///
    /// ⚠ **One instruction for the whole generated palette**, because the palette is open: a table
    /// naming each call would have to be regenerated with the manifest and would say nothing the
    /// manifest does not. The callee travels on the instruction.
    Call,
}

impl Op {
    /// Every op this table names.
    pub const ALL: [Op; 17] = [
        Op::Branch,
        Op::Return,
        Op::For,
        Op::ForEach,
        Op::MakeArray,
        Op::Map,
        Op::Filter,
        Op::Reduce,
        Op::Find,
        Op::Any,
        Op::All,
        Op::Sort,
        Op::IsEmpty,
        Op::Length,
        Op::Literal,
        Op::DialRead,
        Op::Call,
    ];

    /// The op name as it appears in a `.cvs`.
    pub fn name(self) -> &'static str {
        match self {
            Op::Branch => "core.branch",
            Op::Return => "core.return",
            Op::For => "core.for",
            Op::ForEach => "core.for_each",
            Op::MakeArray => "array.make",
            Op::Map => "array.map",
            Op::Filter => "array.filter",
            Op::Reduce => "array.reduce",
            Op::Find => "array.find",
            Op::Any => "array.any",
            Op::All => "array.all",
            Op::Sort => "array.sort",
            Op::IsEmpty => "array.is_empty",
            Op::Length => "array.length",
            Op::Literal => "core.literal",
            Op::DialRead => "core.dial",
            Op::Call => "core.call",
        }
    }

    /// Look one up by name.
    pub fn from_name(name: &str) -> Option<Op> {
        Op::ALL.into_iter().find(|o| o.name() == name)
    }

    /// Does this op take a sub-graph predicate?
    ///
    /// ⚠ **The list is the reason collections are usable at all.** An op that needed a closure would
    /// need a syntax for one, and a visual language has none.
    pub fn takes_subgraph(self) -> bool {
        matches!(
            self,
            Op::Map | Op::Filter | Op::Reduce | Op::Find | Op::Any | Op::All | Op::Sort
        )
    }

    /// Is this a loop, and therefore something that must be provably bounded?
    pub fn is_loop(self) -> bool {
        matches!(self, Op::For | Op::ForEach)
    }

    /// Does control flow pass through this op?
    pub fn is_control_flow(self) -> bool {
        matches!(self, Op::Branch | Op::Return | Op::For | Op::ForEach)
    }

    /// Is this op free of side effects, so a constant input folds to a constant output?
    ///
    /// ⚠ **`DialRead` is *not* pure**, and the exception matters: a dial is a **runtime input** a host
    /// sets before generating, so folding one to its authored default would bake a value the design
    /// spends a section insisting is never baked.
    ///
    /// ⚠ **`Call` is conservatively impure too.** Many generated calls *are* pure — that is what makes
    /// memoization the most valuable optimization available — but purity is a property of the manifest
    /// entry rather than of the instruction, and assuming it here would let dead-code elimination
    /// delete a call that reads the context. Wrong in the cheap direction.
    pub fn is_pure(self) -> bool {
        !matches!(self, Op::Return | Op::DialRead | Op::Call)
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// ⚠ **There is no `While`, and its absence is the feature.**
///
/// Named here so a reader who goes looking finds the reason rather than a gap: an unbounded loop makes
/// termination a property of the *content* rather than of the language, and a graph that can spin is a
/// graph that can hang the editor. Every iteration construct here is bounded by a literal or by a
/// collection's length.
pub const NO_WHILE_LOOP: &str =
    "generation must terminate: every loop is bounded by a literal or a collection's length";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_op_has_a_unique_name_that_round_trips() {
        let mut seen = std::collections::BTreeSet::new();
        for op in Op::ALL {
            assert!(seen.insert(op.name()), "{op} shares a name");
            assert_eq!(Op::from_name(op.name()), Some(op));
        }
        assert_eq!(Op::ALL.len(), 17);
    }

    #[test]
    fn there_is_no_while_loop() {
        // ⚠ Its absence is the feature — see NO_WHILE_LOOP.
        assert!(Op::from_name("core.while").is_none());
        assert!(Op::ALL.iter().all(|o| !o.name().contains("while")));
        assert!(NO_WHILE_LOOP.contains("terminate"));
    }

    #[test]
    fn every_collection_op_that_needs_a_predicate_takes_a_subgraph() {
        for op in [
            Op::Map,
            Op::Filter,
            Op::Reduce,
            Op::Find,
            Op::Any,
            Op::All,
            Op::Sort,
        ] {
            assert!(op.takes_subgraph(), "{op} needs a predicate");
        }
        // ⚠ The 80% case takes none — it is the one a developer reaches for first.
        assert!(!Op::MakeArray.takes_subgraph());
        assert!(!Op::IsEmpty.takes_subgraph());
        assert!(!Op::Length.takes_subgraph());
    }

    #[test]
    fn the_functional_set_covers_what_a_loop_would_otherwise_be_used_for() {
        // ⚠ A developer who reaches for a For to filter a list has been failed by the palette.
        for name in [
            "array.map",
            "array.filter",
            "array.reduce",
            "array.find",
            "array.any",
            "array.all",
            "array.sort",
            "array.make",
        ] {
            assert!(Op::from_name(name).is_some(), "{name} is missing");
        }
    }

    #[test]
    fn both_loops_are_iteration_and_neither_is_unbounded_by_construction() {
        assert!(Op::For.is_loop() && Op::ForEach.is_loop());
        assert!(
            !Op::Map.is_loop(),
            "a map is not a loop the checker must bound"
        );
        assert_eq!(Op::ALL.iter().filter(|o| o.is_loop()).count(), 2);
    }

    #[test]
    fn a_dial_read_is_not_pure_so_it_never_folds_to_its_default() {
        // ⚠ A dial is a runtime input. Folding one would bake a value the design insists is never baked.
        assert!(!Op::DialRead.is_pure());
        assert!(!Op::Return.is_pure());
        assert!(
            !Op::Call.is_pure(),
            "purity lives in the manifest entry, not the instruction — wrong in the cheap direction"
        );
        assert!(Op::Literal.is_pure());
        assert!(Op::Filter.is_pure());
    }

    #[test]
    fn control_flow_is_exactly_branch_return_and_the_two_loops() {
        let flow: Vec<Op> = Op::ALL
            .into_iter()
            .filter(|o| o.is_control_flow())
            .collect();
        assert_eq!(flow, vec![Op::Branch, Op::Return, Op::For, Op::ForEach]);
    }
}
