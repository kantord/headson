use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "headson",
    version,
    about = "Get a small but useful preview of a JSON file"
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
        help = "Do not add newlines in the output"
    )]
    no_newline: bool,
    #[arg(
        short = 'm',
        long = "compact",
        default_value_t = false,
        conflicts_with_all = ["no_space", "no_newline", "indent"],
        help = "Compact output with no added whitespace. Not very human-readable."
    )]
    compact: bool,
    #[arg(
        long = "string-cap",
        default_value_t = 500,
        help = "Maximum string length to display"
    )]
    string_cap: usize,
    #[arg(
        value_name = "INPUT",
        value_hint = clap::ValueHint::FilePath,
        num_args = 0..,
        help = "Optional file paths. If omitted, reads JSON from stdin. Multiple input files are supported."
    )]
    inputs: Vec<PathBuf>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Template {
    Json,
    Pseudo,
    Js,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let render_cfg = get_render_config_from(&cli);
    // Priority config should reflect per-file budget semantics (array caps etc.).
    let priority_cfg = get_priority_config_from(&cli);
    let input_count = if cli.inputs.is_empty() {
        1
    } else {
        cli.inputs.len()
    };
    let effective_budget = cli.budget.saturating_mul(input_count);

    let output = if cli.inputs.len() <= 1 {
        let input_bytes = get_input_single(&cli.inputs)?;
        headson::headson(
            input_bytes,
            &render_cfg,
            &priority_cfg,
            effective_budget,
        )?
    } else {
        let inputs = get_input_many(&cli.inputs)?;
        headson::headson_many(
            inputs,
            &render_cfg,
            &priority_cfg,
            effective_budget,
        )?
    };
    println!("{output}");

    Ok(())
}

fn get_input_single(paths: &[PathBuf]) -> Result<Vec<u8>> {
    // Read input from first file path when provided, otherwise from stdin.
    if let Some(path) = paths.first() {
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

fn get_input_many(paths: &[PathBuf]) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::with_capacity(paths.len());
    for path in paths.iter() {
        let bytes = std::fs::read(path).with_context(|| {
            format!("failed to read input file: {}", path.display())
        })?;
        out.push((path.display().to_string(), bytes));
    }
    Ok(out)
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
    // Optimization: derive a conservative per‑array expansion cap from the output
    // budget to avoid allocating/walking items that could never appear in the
    // final preview. As a simple lower bound, an array of N items needs ~2*N
    // bytes to render (item plus comma), so we cap per‑array expansion at
    // budget/2. This prunes unnecessary work on large inputs without changing
    // output semantics.
    headson::PriorityConfig {
        max_string_graphemes: cli.string_cap,
        array_max_items: (cli.budget / 2).max(1),
    }
}
