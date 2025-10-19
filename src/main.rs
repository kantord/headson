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
        long = "profile",
        default_value_t = false,
        help = "Print timing breakdown to stderr"
    )]
    profile: bool,
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

    // Read input either from a file path (pre-allocated) or from stdin (bytes).
    let input_bytes: Vec<u8> = if let Some(path) = cli.input.as_ref() {
        std::fs::read(path).with_context(|| {
            format!("failed to read input file: {}", path.display())
        })?
    } else {
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .context("failed to read from stdin")?;
        buf
    };

    let template = match cli.template {
        Template::Json => headson::OutputTemplate::Json,
        Template::Pseudo => headson::OutputTemplate::Pseudo,
        Template::Js => headson::OutputTemplate::Js,
    };
    let space = if cli.no_space {
        "".to_string()
    } else {
        " ".to_string()
    };
    let config = headson::RenderConfig {
        template,
        indent_unit: cli.indent.clone(),
        space,
        profile: cli.profile,
    };
    // Derive a conservative per-array cap from the budget: an array of N items
    // minimally needs about 2*N characters (item plus comma) to fit. So cap at budget/2.
    let pq_cfg = headson::PQConfig {
        max_string_graphemes: cli.string_cap,
        array_max_items: (cli.budget / 2).max(1),
    };
    let output = headson::headson(input_bytes, config, &pq_cfg, cli.budget)?;
    println!("{}", output);

    Ok(())
}
