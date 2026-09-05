#![allow(
    clippy::multiple_crate_versions,
    reason = "Dependency graph pulls distinct versions (e.g., yaml-rust2)."
)]
#![cfg_attr(
    test,
    allow(
        clippy::cognitive_complexity,
        clippy::float_cmp,
        reason = "tests may be structurally complex or assert exact float values"
    )
)]
mod cli;
mod session;
mod sorting;

use anyhow::Result;
use clap::{CommandFactory, Parser};

use crate::cli::args::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(shell) = cli.completions {
        eprintln!("generating completions file for {shell:?}");
        cli::args::print_completions(shell, &mut Cli::command());

        return Ok(());
    }

    if let Some(crate::cli::args::TopSubcommand::Explore(ref explore)) =
        cli.subcommand
    {
        let output =
            crate::cli::explore::run_subcommand(&explore.command, &cli)?;
        // Some subcommands (e.g. `explore clear`) succeed with nothing to
        // say; printing would emit a stray blank line.
        if !output.is_empty() {
            println!("{output}");
        }
        return Ok(());
    }

    let (output, warnings) = crate::cli::run::run(&cli)?;
    println!("{output}");

    for warning in warnings {
        eprintln!("{warning}");
    }

    Ok(())
}
