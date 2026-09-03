//! `cv` — the headless CycleVania CLI.
//!
//! ⚠ **The shell every later apparatus milestone hangs from.** The scenario runner, the determinism
//! harness and CI all invoke this, so it exists before any of them rather than being discovered missing
//! by the first one that needs it.
//!
//! ⚠ **Native Rust over the core, never through the TS bindings.** The CLI must stay usable while the
//! bindings are mid-change, because it is the tool used to *debug* them.
//!
//! ```text
//! cv check       <project>                 compile everything, report, generate nothing
//! cv generate    <project> --seed S        the recipe and the roll
//! cv determinism <project> --seeds N       the soak
//! cv trace       <project> --seed S        what was decided, and why
//! ```
//!
//! `--json` on any of them prints one machine-readable object instead of text.

mod project;
mod run;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "cv", version, about = "CycleVania SDK CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Print one machine-readable object instead of text.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Compile all content and report, without generating.
    Check {
        /// The `.cvproj` descriptor.
        project: PathBuf,
    },
    /// Generate a world.
    Generate {
        /// The `.cvproj` descriptor.
        project: PathBuf,
        /// The seed. Text, so it survives being written into a bug report.
        #[arg(long, default_value = "default")]
        seed: String,
    },
    /// Run the determinism soak.
    Determinism {
        /// The `.cvproj` descriptor.
        project: PathBuf,
        /// How many seeds to try.
        #[arg(long, default_value_t = 64)]
        seeds: u32,
    },
    /// Emit the trace for one seed.
    Trace {
        /// The `.cvproj` descriptor.
        project: PathBuf,
        /// The seed.
        #[arg(long, default_value = "default")]
        seed: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        println!("cv {} — run `cv --help`.", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    };

    let report = match command {
        Command::Check { project } => run::check(&project),
        Command::Generate { project, seed } => run::generate(&project, &seed),
        Command::Determinism { project, seeds } => run::determinism(&project, seeds),
        Command::Trace { project, seed } => run::trace(&project, &seed),
    };

    // ⚠ **One stream, and everything on it is the report.** A tool that split findings onto stderr
    // would make `cv check --json > out.json` produce a file missing exactly the part a reader wanted
    // — the findings are *in* the object, so redirecting stdout captures the whole answer.
    if cli.json {
        println!("{}", report.json());
    } else {
        print!("{}", report.text());
    }
    ExitCode::from(report.exit.code())
}
