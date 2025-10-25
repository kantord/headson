use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

type InputEntry = (String, Vec<u8>);
type InputEntries = Vec<InputEntry>;
type IgnoreNotices = Vec<String>;

#[derive(Parser, Debug)]
#[command(
    name = "headson",
    version,
    about = "Get a small but useful preview of a JSON file"
)]
struct Cli {
    #[arg(short = 'n', long = "budget", conflicts_with = "global_budget")]
    budget: Option<usize>,
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
        short = 'N',
        long = "global-budget",
        value_name = "BYTES",
        conflicts_with = "budget",
        help = "Total output budget across all inputs; useful to keep multiple files within a fixed overall output size (may omit entire files)."
    )]
    global_budget: Option<usize>,
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

#[allow(
    clippy::cognitive_complexity,
    reason = "top-level CLI orchestration keeps control flow clear; extracted helpers would be noisy here"
)]
fn main() -> Result<()> {
    let cli = Cli::parse();

    let render_cfg = get_render_config_from(&cli);
    let mut ignore_notices: IgnoreNotices = Vec::new();

    let output = if cli.inputs.is_empty() {
        // Stdin mode
        let input_bytes = get_input_single(&cli.inputs)?;
        let input_count = 1usize;
        let effective_budget = if let Some(g) = cli.global_budget {
            g
        } else {
            let per_file = cli.budget.unwrap_or(500);
            per_file.saturating_mul(input_count)
        };
        let per_file_for_priority = (effective_budget / input_count).max(1);
        let priority_cfg = get_priority_config(per_file_for_priority, &cli);
        headson::headson(
            input_bytes,
            &render_cfg,
            &priority_cfg,
            effective_budget,
        )?
    } else {
        // Paths mode (single or multiple): skip dirs/binaries uniformly.
        let (entries, ignored) = get_input_many(&cli.inputs)?;
        ignore_notices = ignored;
        let included = entries.len();
        let input_count = included.max(1);
        let effective_budget = if let Some(g) = cli.global_budget {
            g
        } else {
            let per_file = cli.budget.unwrap_or(500);
            per_file.saturating_mul(input_count)
        };
        let per_file_for_priority = (effective_budget / input_count).max(1);
        let priority_cfg = get_priority_config(per_file_for_priority, &cli);
        if cli.inputs.len() > 1 {
            headson::headson_many(
                entries,
                &render_cfg,
                &priority_cfg,
                effective_budget,
            )?
        } else if included == 0 {
            String::new()
        } else {
            let bytes = entries.into_iter().next().unwrap().1;
            headson::headson(
                bytes,
                &render_cfg,
                &priority_cfg,
                effective_budget,
            )?
        }
    };
    println!("{output}");

    for notice in ignore_notices {
        eprintln!("{notice}");
    }

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

fn read_file_with_nul_check(path: &Path) -> Result<Result<Vec<u8>, ()>> {
    // Read the file while checking for NUL bytes. If a NUL is seen, treat as binary
    // and stop early to avoid unnecessary I/O/CPU.
    let file = File::open(path).with_context(|| {
        format!("failed to open input file: {}", path.display())
    })?;
    let mut reader = io::BufReader::with_capacity(64 * 1024, file);
    let mut buf = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk).with_context(|| {
            format!("failed to read input file: {}", path.display())
        })?;
        if n == 0 {
            break;
        }
        // If chunk contains NUL, consider it binary and stop reading further.
        if memchr::memchr(0, &chunk[..n]).is_some() {
            return Ok(Err(()));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(Ok(buf))
}

fn get_input_many(paths: &[PathBuf]) -> Result<(InputEntries, IgnoreNotices)> {
    let mut out: InputEntries = Vec::with_capacity(paths.len());
    let mut ignored: IgnoreNotices = Vec::new();
    for path in paths.iter() {
        let display = path.display().to_string();
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.is_dir() {
                ignored.push(format!("Ignored directory: {display}"));
                continue;
            }
        }
        if let Ok(bytes) = read_file_with_nul_check(path)? {
            out.push((display, bytes))
        } else {
            ignored.push(format!("Ignored binary file: {display}"));
            continue;
        }
    }
    Ok((out, ignored))
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

fn get_priority_config(
    per_file_budget: usize,
    cli: &Cli,
) -> headson::PriorityConfig {
    // Optimization: derive a conservative per‑array expansion cap from the output
    // budget to avoid allocating/walking items that could never appear in the
    // final preview. As a simple lower bound, an array of N items needs ~2*N
    // bytes to render (item plus comma), so we cap per‑array expansion at
    // budget/2. This prunes unnecessary work on large inputs without changing
    // output semantics.
    headson::PriorityConfig {
        max_string_graphemes: cli.string_cap,
        array_max_items: (per_file_budget / 2).max(1),
    }
}
