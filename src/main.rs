#![warn(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::bool_comparison,
    clippy::branches_sharing_code
)]

use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "headson",
    version,
    about = "Read JSON from stdin and render a prioritized, budget‑constrained preview"
)]
struct Cli {
    #[arg(short = 'n', long = "budget", default_value_t = 500)]
    budget: usize,
    #[arg(short = 'f', long = "template", value_enum, default_value_t = Template::Pseudo)]
    template: Template,
    #[arg(long = "indent", default_value = "  ")]
    indent: String,
    #[arg(long = "no-space", default_value_t = false)]
    no_space: bool,
    #[arg(
        long = "no-newline",
        default_value_t = false,
        help = "Remove newlines in output (one-line)"
    )]
    no_newline: bool,
    #[arg(
        short = 'm',
        long = "compact",
        default_value_t = false,
        conflicts_with_all = ["no_space", "no_newline", "indent"],
        help = "Compact output: disables indentation, spaces after colons, and newlines"
    )]
    compact: bool,
    #[arg(
        long = "string-cap",
        default_value_t = 500,
        help = "Maximum graphemes to expand per string in PQ build"
    )]
    string_cap: usize,
    #[arg(
        long = "input",
        help = "Read JSON directly from a file path instead of stdin"
    )]
    input: Option<PathBuf>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Template {
    Json,
    Pseudo,
    Js,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let input_bytes = get_input(cli.input.as_ref())?;
    let render_cfg = get_render_config_from(&cli);
    let priority_cfg = get_priority_config_from(&cli);

    let output =
        headson::headson(input_bytes, &render_cfg, &priority_cfg, cli.budget)?;
    println!("{}", output);

    Ok(())
}

fn get_input(path: Option<&PathBuf>) -> Result<Vec<u8>> {
    // Read input either from a file path (pre-allocated) or from stdin (bytes).
    if let Some(path) = path {
        std::fs::read(path).with_context(|| {
            format!("failed to read input file: {}", path.display())
        })
    } else {
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .context("failed to read from stdin")?;
        Ok(buf)
    }
}

fn get_render_config_from(cli: &Cli) -> headson::RenderConfig {
    let template = match cli.template {
        Template::Json => headson::OutputTemplate::Json,
        Template::Pseudo => headson::OutputTemplate::Pseudo,
        Template::Js => headson::OutputTemplate::Js,
    };
    let space = if cli.compact || cli.no_space { "" } else { " " }.to_string();
    let newline = if cli.compact || cli.no_newline {
        ""
    } else {
        "\n"
    }
    .to_string();
    let indent_unit = if cli.compact {
        String::new()
    } else {
        cli.indent.clone()
    };

    headson::RenderConfig {
        template,
        indent_unit,
        space,
        newline,
    }
}

fn get_priority_config_from(cli: &Cli) -> headson::PriorityConfig {
    // Derive a conservative per-array cap from the budget: an array of N items
    // minimally needs about 2*N characters (item plus comma) to fit. So cap at budget/2.
    headson::PriorityConfig {
        max_string_graphemes: cli.string_cap,
        array_max_items: (cli.budget / 2).max(1),
    }
}
