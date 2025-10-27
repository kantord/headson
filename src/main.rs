use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
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
    #[arg(short = 'n', long = "budget")]
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
        help = "Total output budget across all inputs. When combined with --budget, the effective global limit is the smaller of the two."
    )]
    global_budget: Option<usize>,
    #[arg(
        long = "tail",
        default_value_t = false,
        help = "Prefer the end of arrays when truncating. Strings unaffected; JSON stays strict."
    )]
    tail: bool,
    #[arg(
        long = "head",
        default_value_t = false,
        conflicts_with = "tail",
        help = "Prefer the beginning of arrays when truncating (keep first N)."
    )]
    head: bool,
    #[arg(
        long = "color",
        action = ArgAction::SetTrue,
        conflicts_with = "no_color",
        help = "Force enable ANSI colors in output"
    )]
    color: bool,
    #[arg(
        long = "no-color",
        action = ArgAction::SetTrue,
        conflicts_with = "color",
        help = "Disable ANSI colors in output"
    )]
    no_color: bool,
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
    match (cli.global_budget, cli.budget) {
        (Some(g), Some(n)) => g.min(n.saturating_mul(input_count)),
        (Some(g), None) => g,
        (None, Some(n)) => n.saturating_mul(input_count),
        (None, None) => 500usize.saturating_mul(input_count),
    }
}

fn compute_priority(
    cli: &Cli,
    effective_budget: usize,
    input_count: usize,
) -> headson::PriorityConfig {
    let per_file_for_priority =
        if cli.global_budget.is_some() && cli.budget.is_some() {
            // When both limits are provided, base per-file heuristics on the per-file
            // budget but also respect the effective per-file slice of the final global.
            let eff_per_file = (effective_budget / input_count.max(1)).max(1);
            cli.budget.unwrap().min(eff_per_file).max(1)
        } else {
            (effective_budget / input_count.max(1)).max(1)
        };
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
    fn to_output_template(t: Template) -> headson::OutputTemplate {
        match t {
            Template::Json => headson::OutputTemplate::Json,
            Template::Pseudo => headson::OutputTemplate::Pseudo,
            Template::Js => headson::OutputTemplate::Js,
        }
    }
    fn color_mode_from_flags(cli: &Cli) -> headson::ColorMode {
        if cli.color {
            headson::ColorMode::On
        } else if cli.no_color {
            headson::ColorMode::Off
        } else {
            headson::ColorMode::Auto
        }
    }

    let template = to_output_template(cli.template);
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
    let color_mode = color_mode_from_flags(cli);

    headson::RenderConfig {
        template,
        indent_unit,
        space,
        newline,
        prefer_tail_arrays: cli.tail,
        color_mode,
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
        array_bias: headson::ArrayBias::HeadMidTail,
        array_sampler: if cli.tail {
            headson::ArraySamplerStrategy::Tail
        } else if cli.head {
            headson::ArraySamplerStrategy::Head
        } else {
            headson::ArraySamplerStrategy::Default
        },
    }
}
