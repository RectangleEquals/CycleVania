//! `cv` — the headless CycleVania CLI. **M00: `--version` and a `build --dry` no-op.**
//! Real subcommands (generate / validate / soak / report) arrive with the pipeline (M08+).

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cv", version, about = "CycleVania SDK CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the generation pipeline (M00: a dry no-op stub).
    Build {
        /// Parse + validate the plan without producing output.
        #[arg(long)]
        dry: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Build { dry }) => {
            if dry {
                println!(
                    "cv build --dry: pipeline is a no-op (M00 bootstrap). \
                     core {}, determinism {}.",
                    cv_core::version(),
                    cv_determinism::version(),
                );
            } else {
                println!(
                    "cv build: the pipeline is not implemented yet (M00 bootstrap). Try --dry."
                );
            }
        }
        None => {
            println!("cv {} — run `cv --help`.", env!("CARGO_PKG_VERSION"));
        }
    }
}
