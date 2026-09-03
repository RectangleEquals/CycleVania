//! cv-vm — the owned bytecode virtual machine that runs compiled **Schematics** in-process, on both
//! native and WASM. No Rhai; no host FFI beyond the sanctioned Context surface.
//!
//! ⚠ **The input is a compiled node graph, not a text file.** CVScript is a *visual* system: a
//! developer authors Schematics as graphs, and those are compiled to the instruction set this executes.
//! There is no scripting-language source artifact to load.
//!
//! ⚠ **Bytecode is an intermediate, never an artifact.** It never ships as individual files, is not
//! committed or distributed, and goes straight into one cooked `game.cvpak`. For an open-source build
//! it is arguably in-memory only, which is how this crate treats it.
//!
//! ⚠ **If it is ever written to disk it is `.cvo`** — CycleVania *Object*, in the compiler's sense —
//! **under `build/`, never under `content/`.** Writing an intermediate into the content root would put
//! a build product under version control, into the asset globs, and into the cook's walk of authored
//! roots: three failures from one misplaced file.
//!
//! ⚠ **Not named after CVB.** CVB is the block *notation* — `Begin X … End X`, `Key=Value`, `Pin (…)`
//! — in which `.cvs`, `.cvspine` and `.cvstate` are three *separate* formats written. An extension
//! derived from it would read as *"a CVB file"*, a category that does not exist. Bytecode is not
//! written in the notation and has no relationship to it.
//!
//! # What lives here
//!
//! * [`ops`] — the op table: control flow, collections and literals, the part of the palette the
//!   manifest cannot describe.
//! * [`ir`] — the typed instruction set. ⚠ **The ISA belongs to the machine, not to the compiler** —
//!   defining it in `cv-compile` would make `cv-core`, which embeds this crate, depend transitively on
//!   a text parser.
//! * [`memo`] — memoization keyed on **recorded** context reads.
//! * [`exec`] — the interpreter, its context surface, and cancellation.
//!
//! ⚠ **Hooks are overrides, never callback fields.** The binding contract forbids closures and trait
//! objects across the seam, so a hook is a graph a developer drew — compiled, and dispatched through a
//! table this crate owns. That is what makes the same content run identically on native and WASM.

pub mod exec;
pub mod ir;
pub mod memo;
pub mod ops;

pub use exec::{Cancel, Context, Outcome, TableContext, Trap, Val, Vm};
pub use ir::{Const, Instr, Program, Ty};
pub use memo::{Channel, Key, Memo, Miss, Read, Recording};
pub use ops::Op;

/// This crate's version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::version().is_empty());
    }
}
