#![allow(
    clippy::multiple_crate_versions,
    reason = "Dependency graph pulls distinct versions (e.g., yaml-rust2)."
)]
mod cli;
mod sorting;

use anyhow::Result;
use clap::Parser;

use crate::cli::args::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let (output, ignore_notices) = crate::cli::run::run(&cli)?;
    println!("{output}");

    for notice in ignore_notices {
        eprintln!("{notice}");
    }

    Ok(())
}
