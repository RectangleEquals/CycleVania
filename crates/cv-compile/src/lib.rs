//! **The schematic compiler** — parse, analyse, lower, emit.
//!
//! ```text
//! .cvs ─► Parse ─► Analyse ─► Lower ─► Bytecode ─► VM (embedded in the core)
//!         (graph)  (checks)   (IR)     (owned)      (dispatches api calls)
//! ```
//!
//! ⚠ **Parse is a schema-validated load, not a grammar** — that work lives in `cv-cvb`, and it is
//! smaller than a lexer plus a parser because the notation was shaped to make it so. What this crate
//! owns is everything after: the checks, the instruction set, and the order.
//!
//! ⚠ **Our own small deterministic bytecode, not an embedded third-party language.** The entire reason
//! this system exists is so a developer need not learn one, and building something meant to be replaced
//! later is wasted work.
//!
//! # The compile either produces a program or a list of findings
//!
//! ⚠ **Never both, and never a program with errors in it.** A compiler that returned a partial artifact
//! alongside its complaints invites a caller to use the artifact, and the one thing worse than a build
//! that fails is a build that half-succeeds.

#![forbid(unsafe_code)]

pub mod analyse;
pub mod lower;

pub use analyse::{analyse, Analysis, Finding, Severity};
pub use lower::{lower, LowerError};

// ⚠ **The instruction set belongs to the VM, not to the compiler.** A compiler targets an ISA; the ISA
// is the machine's. Defining it here would have made `cv-core` — which embeds the VM — depend
// transitively on a text parser, which is exactly the dependency `cv-cvb` was placed below the core to
// avoid.
pub use cv_vm::ir::{Const, Instr, Program, Ty};
pub use cv_vm::ops::Op;

use cv_cvb::parse::Block;

/// What a compile produced.
#[derive(Clone, Debug, PartialEq)]
pub enum Compiled {
    /// One program per hook, plus anything worth saying that did not stop the compile.
    Ok {
        programs: Vec<Program>,
        findings: Analysis,
    },
    /// Nothing was emitted.
    Failed { findings: Analysis },
}

impl Compiled {
    /// Did it produce anything?
    pub fn succeeded(&self) -> bool {
        matches!(self, Compiled::Ok { .. })
    }

    /// Everything analysis had to say, either way.
    pub fn findings(&self) -> &Analysis {
        match self {
            Compiled::Ok { findings, .. } | Compiled::Failed { findings } => findings,
        }
    }

    /// The programs, when there are any.
    pub fn programs(&self) -> &[Program] {
        match self {
            Compiled::Ok { programs, .. } => programs,
            Compiled::Failed { .. } => &[],
        }
    }
}

/// Compile a parsed schematic.
///
/// ⚠ **Optimizations run after lowering and before anything is returned**, so a caller cannot receive
/// an unoptimized program by forgetting a step — and the optimized form is the only form there is.
pub fn compile(root: &Block) -> Compiled {
    let mut findings = analyse(root);
    if findings.failed() {
        return Compiled::Failed { findings };
    }

    let mut programs = Vec::new();
    for graph in root.blocks("Graph") {
        match lower(graph) {
            Ok(mut program) => {
                program.fold_constants();
                program.eliminate_dead_code();
                programs.push(program);
            }
            Err(e) => {
                let node = graph
                    .header_get("Id")
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                findings.findings.push(Finding {
                    severity: Severity::Error,
                    node,
                    pin: None,
                    message: e.to_string(),
                    hint: None,
                });
            }
        }
    }

    if findings.failed() {
        return Compiled::Failed { findings };
    }
    // ⚠ **Sorted by hook, not left in document order.** Two schematics whose graphs were authored in
    // a different order are the same class, and an artifact that recorded the difference would make a
    // reordering in the editor produce a different build.
    programs.sort_by(|a, b| a.hook.cmp(&b.hook));
    Compiled::Ok { programs, findings }
}

/// This crate's version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cv_cvb::parse::parse;

    #[test]
    fn version_is_set() {
        assert!(!version().is_empty());
    }

    #[test]
    fn a_failed_compile_emits_no_program_at_all() {
        // ⚠ The one thing worse than a build that fails is a build that half-succeeds.
        let doc = parse(
            "Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=s\n   \
             Begin Graph Name=\"nosuchhook\" Role=Hook Id=grf\n   End Graph\nEnd Schematic\n",
        )
        .unwrap();
        let got = compile(&doc);
        assert!(!got.succeeded());
        assert!(got.programs().is_empty());
        assert!(got.findings().failed());
    }

    #[test]
    fn a_clean_schematic_compiles_to_one_program_per_graph() {
        let doc = parse(
            "Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=s\n   \
             Begin Graph Name=\"grants\" Role=Hook Id=grf_1\n      \
             Begin Node Id=n_1 Op=array.make Pos=(0,0)\n         \
             Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/Object'>)\n      End Node\n   End Graph\n   \
             Begin Graph Name=\"requires\" Role=Hook Id=grf_2\n      \
             Begin Node Id=n_2 Op=array.make Pos=(0,0)\n         \
             Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/Object'>)\n      End Node\n   End Graph\n\
             End Schematic\n",
        )
        .unwrap();
        let got = compile(&doc);
        assert!(got.succeeded(), "{:?}", got.findings());
        assert_eq!(got.programs().len(), 2);
        let hooks: Vec<&str> = got.programs().iter().map(|p| p.hook.as_str()).collect();
        assert_eq!(hooks, vec!["grants", "requires"]);
    }

    #[test]
    fn a_lint_does_not_stop_a_compile() {
        let doc = parse(
            "Begin Schematic Version=1 Path=/Content/Items/hookShot Extends=Kind'/Core/Item' Id=s\n\
             End Schematic\n",
        )
        .unwrap();
        let got = compile(&doc);
        assert!(got.succeeded());
        assert!(!got.findings().of(Severity::Lint).is_empty());
    }
}
