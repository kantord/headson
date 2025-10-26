use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use content_inspector::{ContentType, inspect};

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
        long = "tail",
        default_value_t = false,
        help = "Prefer the end of arrays when truncating. Strings unaffected; JSON stays strict."
    )]
    tail: bool,
    #[arg(
        value_name = "INPUT",
        value_hint = clap::ValueHint::FilePath,
        num_args = 0..,
        help = "Optional file paths. If omitted, reads JSON from stdin. Multiple input files are supported. Directories and binary files are ignored with a notice on stderr."
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
    let (output, ignore_notices) = if cli.inputs.is_empty() {
        (run_from_stdin(&cli, &render_cfg)?, Vec::new())
    } else {
        run_from_paths(&cli, &render_cfg)?
    };
    println!("{output}");

    for notice in ignore_notices {
        eprintln!("{notice}");
    }

    Ok(())
}

fn compute_effective_budget(cli: &Cli, input_count: usize) -> usize {
    if let Some(g) = cli.global_budget {
        g
    } else {
        let per_file = cli.budget.unwrap_or(500);
        per_file.saturating_mul(input_count)
    }
}

fn compute_priority(
    cli: &Cli,
    effective_budget: usize,
    input_count: usize,
) -> headson::PriorityConfig {
    let per_file_for_priority = (effective_budget / input_count.max(1)).max(1);
    get_priority_config(per_file_for_priority, cli)
}

fn run_from_stdin(
    cli: &Cli,
    render_cfg: &headson::RenderConfig,
) -> Result<String> {
    let input_bytes = read_stdin()?;
    let input_count = 1usize;
    let eff = compute_effective_budget(cli, input_count);
    let prio = compute_priority(cli, eff, input_count);
    headson::headson(input_bytes, render_cfg, &prio, eff)
}

fn run_from_paths(
    cli: &Cli,
    render_cfg: &headson::RenderConfig,
) -> Result<(String, IgnoreNotices)> {
    let (entries, ignored) = ingest_paths(&cli.inputs)?;
    let included = entries.len();
    let input_count = included.max(1);
    let eff = compute_effective_budget(cli, input_count);
    let prio = compute_priority(cli, eff, input_count);
    if cli.inputs.len() > 1 {
        let out = headson::headson_many(entries, render_cfg, &prio, eff)?;
        Ok((out, ignored))
    } else if included == 0 {
        Ok((String::new(), ignored))
    } else {
        let bytes = entries.into_iter().next().unwrap().1;
        let out = headson::headson(bytes, render_cfg, &prio, eff)?;
        Ok((out, ignored))
    }
}

fn read_stdin() -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    io::stdin()
        .read_to_end(&mut buf)
        .context("failed to read from stdin")?;
    Ok(buf)
}

fn sniff_then_read_text(path: &Path) -> Result<Option<Vec<u8>>> {
    // Inspect the first chunk with content_inspector; if it looks binary, skip.
    // Otherwise, read the remainder without further inspection for speed.
    const CHUNK: usize = 64 * 1024;
    let file = File::open(path).with_context(|| {
        format!("failed to open input file: {}", path.display())
    })?;
    let meta_len = file.metadata().ok().map(|m| m.len());
    let mut reader = io::BufReader::with_capacity(CHUNK, file);

    let mut first = [0u8; CHUNK];
    let n = reader.read(&mut first).with_context(|| {
        format!("failed to read input file: {}", path.display())
    })?;
    if n == 0 {
        return Ok(Some(Vec::new()));
    }
    if matches!(inspect(&first[..n]), ContentType::BINARY) {
        return Ok(None);
    }

    // Preallocate buffer: first chunk + estimated remainder (capped)
    let mut buf = Vec::with_capacity(
        n + meta_len
            .map(|m| m.saturating_sub(n as u64) as usize)
            .unwrap_or(0)
            .min(8 * 1024 * 1024),
    );
    buf.extend_from_slice(&first[..n]);
    reader.read_to_end(&mut buf).with_context(|| {
        format!("failed to read input file: {}", path.display())
    })?;
    Ok(Some(buf))
}

fn ingest_paths(paths: &[PathBuf]) -> Result<(InputEntries, IgnoreNotices)> {
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
        if let Some(bytes) = sniff_then_read_text(path)? {
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
        prefer_tail_arrays: cli.tail,
    }
}

fn get_priority_config(
    per_file_budget: usize,
    cli: &Cli,
) -> headson::PriorityConfig {
    headson::PriorityConfig {
        max_string_graphemes: cli.string_cap,
        array_max_items: (per_file_budget / 2).max(1),
        prefer_tail_arrays: cli.tail,
        array_bias: headson::ArrayBias::Head,
    }
}
