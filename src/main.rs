#![allow(
    clippy::multiple_crate_versions,
    reason = "Dependency graph pulls distinct versions (e.g., yaml-rust2)."
)]
mod cli;
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

    let (output, warnings) = crate::cli::run::run(&cli)?;
    println!("{output}");

    for warning in warnings {
        eprintln!("{warning}");
    }

    Ok(())
}
