#![allow(
    clippy::multiple_crate_versions,
    reason = "Dependency graph pulls distinct versions (e.g., yaml-rust2)."
)]
mod sorting;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{ArgAction, Parser, ValueEnum};
use content_inspector::{ContentType, inspect};
use sorting::sort_paths_for_fileset;

type InputEntry = (String, Vec<u8>);
type InputEntries = Vec<InputEntry>;
type IgnoreNotices = Vec<String>;

#[derive(Parser, Debug)]
#[command(
    name = "hson",
    version,
    about = "Get a small but useful preview of JSON or YAML"
)]
struct Cli {
    #[arg(short = 'c', long = "bytes")]
    bytes: Option<usize>,
    #[arg(
        short = 'u',
        long = "chars",
        value_name = "CHARS",
        help = "Per-file Unicode character budget (adds up across files if no global chars limit)"
    )]
    chars: Option<usize>,
    #[arg(
        short = 'n',
        long = "lines",
        value_name = "LINES",
        help = "Per-file line budget (adds up across files if --global-lines not set)"
    )]
    lines: Option<usize>,
    #[arg(
        short = 'f',
        long = "format",
        value_enum,
        default_value_t = OutputFormat::Auto,
        help = "Output format: auto|json|yaml|text (filesets: auto is per-file)."
    )]
    format: OutputFormat,
    #[arg(
        short = 't',
        long = "template",
        value_enum,
        default_value_t = StyleArg::Default,
        help = "Output style: strict|default|detailed."
    )]
    style: StyleArg,
    #[arg(long = "indent", default_value = "  ")]
    indent: String,
    #[arg(long = "no-space", default_value_t = false)]
    no_space: bool,
    #[arg(
        long = "no-newline",
        default_value_t = false,
        conflicts_with_all = ["lines", "global_lines"],
        help = "Do not add newlines in the output. Incompatible with --lines/--global-lines."
    )]
    no_newline: bool,
    #[arg(
        long = "no-header",
        default_value_t = false,
        help = "Suppress fileset section headers in the output"
    )]
    no_header: bool,
    #[arg(
        long = "no-sort",
        default_value_t = false,
        help = "Keep input order for filesets (skip frecency/mtime sorting)."
    )]
    no_sort: bool,
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
        short = 'C',
        long = "global-bytes",
        value_name = "BYTES",
        help = "Total byte budget across all inputs. When combined with --bytes, the effective global limit is the smaller of the two."
    )]
    global_bytes: Option<usize>,
    #[arg(
        short = 'N',
        long = "global-lines",
        value_name = "LINES",
        help = "Total line budget across all inputs"
    )]
    global_lines: Option<usize>,
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
        help = "Optional file paths. If omitted, reads input from stdin. Multiple input files are supported. Directories and binary files are ignored with a notice on stderr."
    )]
    inputs: Vec<PathBuf>,
    #[arg(
        short = 'i',
        long = "input-format",
        value_enum,
        default_value_t = InputFormat::Json,
        help = "Input ingestion format: json|yaml|text."
    )]
    input_format: InputFormat,
    #[arg(
        long = "debug",
        default_value_t = false,
        help = "Dump pruned internal tree (JSON) to stderr for the final render attempt"
    )]
    debug: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFormat {
    Auto,
    Json,
    Yaml,
    Text,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum StyleArg {
    Strict,
    Default,
    Detailed,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum InputFormat {
    Json,
    Yaml,
    Text,
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

// Build budgets from CLI flags. If only line caps are provided, avoid imposing
// the default byte cap; keep the 500-byte default only when neither lines nor
// chars nor bytes are specified. If any byte-related flag is present, enforce bytes.
fn make_budgets(
    cli: &Cli,
    eff_bytes: usize,
    eff_lines: Option<usize>,
    eff_chars: Option<usize>,
) -> headson::Budgets {
    let any_bytes = cli.bytes.is_some() || cli.global_bytes.is_some();
    let any_lines = cli.lines.is_some() || cli.global_lines.is_some();
    let any_chars = cli.chars.is_some();

    // Apply default 500-byte only when no explicit budgets provided.
    let byte_budget = if any_bytes {
        Some(eff_bytes)
    } else if any_lines || any_chars {
        None
    } else {
        Some(eff_bytes)
    };
    headson::Budgets {
        byte_budget,
        char_budget: if any_chars { eff_chars } else { None },
        line_budget: eff_lines,
    }
}

fn compute_effective_bytes(cli: &Cli, input_count: usize) -> usize {
    match (cli.global_bytes, cli.bytes) {
        (Some(g), Some(n)) => g.min(n.saturating_mul(input_count)),
        (Some(g), None) => g,
        (None, Some(n)) => n.saturating_mul(input_count),
        (None, None) => 500usize.saturating_mul(input_count),
    }
}

fn compute_effective_chars(cli: &Cli, input_count: usize) -> Option<usize> {
    cli.chars.map(|n| n.saturating_mul(input_count))
}

fn compute_effective_lines(cli: &Cli, input_count: usize) -> Option<usize> {
    match (cli.global_lines, cli.lines) {
        (Some(g), Some(n)) => Some(g.min(n.saturating_mul(input_count))),
        (Some(g), None) => Some(g),
        (None, Some(n)) => Some(n.saturating_mul(input_count)),
        (None, None) => None,
    }
}

fn compute_priority(
    cli: &Cli,
    effective_bytes: usize,
    effective_chars: Option<usize>,
    input_count: usize,
) -> headson::PriorityConfig {
    // Choose a unit for heuristics: prefer bytes if present; else chars if present; else default bytes.
    let chosen_global = if cli.bytes.is_some() || cli.global_bytes.is_some() {
        effective_bytes
    } else if let Some(c) = effective_chars {
        c
    } else {
        effective_bytes
    };
    let per_file_for_priority = (chosen_global / input_count.max(1)).max(1);
    get_priority_config(per_file_for_priority, cli)
}

fn detect_fileset_input_kind(name: &str) -> headson::FilesetInputKind {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        headson::FilesetInputKind::Yaml
    } else if lower.ends_with(".json") {
        headson::FilesetInputKind::Json
    } else {
        let atomic = headson::extensions::is_code_like_name(&lower);
        headson::FilesetInputKind::Text {
            atomic_lines: atomic,
        }
    }
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Keeps ingest + final render + debug plumbing co-located"
)]
fn run_from_stdin(
    cli: &Cli,
    render_cfg: &headson::RenderConfig,
) -> Result<String> {
    let input_bytes = read_stdin()?;
    let input_count = 1usize;
    let eff = compute_effective_bytes(cli, input_count);
    let eff_chars = compute_effective_chars(cli, input_count);
    let eff_lines = compute_effective_lines(cli, input_count);
    let prio = compute_priority(cli, eff, eff_chars, input_count);
    let mut cfg = render_cfg.clone();
    // Resolve effective output template for stdin:
    cfg.template = resolve_effective_template_for_stdin(cli.format, cfg.style);
    let budgets = make_budgets(cli, eff, eff_lines, eff_chars);
    // Enable free string prefix when in line-only mode
    if budgets.byte_budget.is_none()
        && budgets.char_budget.is_none()
        && budgets.line_budget.is_some()
    {
        cfg.string_free_prefix_graphemes = Some(40);
    }
    let out = match cli.input_format {
        InputFormat::Json => {
            headson::headson_with_budgets(input_bytes, &cfg, &prio, budgets)?
        }
        InputFormat::Yaml => headson::headson_yaml_with_budgets(
            input_bytes,
            &cfg,
            &prio,
            budgets,
        )?,
        InputFormat::Text => headson::headson_text_with_budgets(
            input_bytes,
            &cfg,
            &prio,
            budgets,
        )?,
    };
    Ok(out)
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "Keeps fileset ingest/selection/render + debug in one place"
)]
fn run_from_paths(
    cli: &Cli,
    render_cfg: &headson::RenderConfig,
) -> Result<(String, IgnoreNotices)> {
    let sorted_inputs = if cli.inputs.len() > 1 && !cli.no_sort {
        sort_paths_for_fileset(&cli.inputs)
    } else {
        cli.inputs.clone()
    };
    if std::env::var_os("HEADSON_FRECEN_TRACE").is_some() {
        eprintln!("run_from_paths sorted_inputs={sorted_inputs:?}");
    }
    let (entries, ignored) = ingest_paths(&sorted_inputs)?;
    if std::env::var_os("HEADSON_FRECEN_TRACE").is_some() {
        eprintln!(
            "run_from_paths ingested={:?}",
            entries.iter().map(|(n, _)| n).collect::<Vec<_>>()
        );
    }
    let included = entries.len();
    let input_count = included.max(1);
    let eff = compute_effective_bytes(cli, input_count);
    let eff_chars = compute_effective_chars(cli, input_count);
    let eff_lines = compute_effective_lines(cli, input_count);
    let prio = compute_priority(cli, eff, eff_chars, input_count);
    if cli.inputs.len() > 1 {
        if !matches!(cli.format, OutputFormat::Auto) {
            bail!(
                "--format cannot be customized for filesets; remove it or set to auto"
            );
        }
        let mut cfg = render_cfg.clone();
        // Filesets always render with per-file auto templates.
        cfg.template = headson::OutputTemplate::Auto;
        let budgets = make_budgets(cli, eff, eff_lines, eff_chars);
        if budgets.byte_budget.is_none()
            && budgets.char_budget.is_none()
            && budgets.line_budget.is_some()
        {
            cfg.string_free_prefix_graphemes = Some(40);
        }
        let files: Vec<headson::FilesetInput> = entries
            .into_iter()
            .map(|(name, bytes)| {
                let kind = detect_fileset_input_kind(&name);
                headson::FilesetInput { name, bytes, kind }
            })
            .collect();
        let out = headson::headson_fileset_multi_with_budgets(
            files, &cfg, &prio, budgets,
        )?;
        return Ok((out, ignored));
    }

    if included == 0 {
        return Ok((String::new(), ignored));
    }

    let (name, bytes) = entries.into_iter().next().unwrap();
    // Single file: pick ingest and output template per CLI format+style.
    let lower = name.to_ascii_lowercase();
    let is_yaml_ext = lower.ends_with(".yaml") || lower.ends_with(".yml");
    let chosen_input = match cli.format {
        OutputFormat::Auto => {
            if is_yaml_ext {
                InputFormat::Yaml
            } else if lower.ends_with(".json") {
                InputFormat::Json
            } else {
                InputFormat::Text
            }
        }
        _ => cli.input_format,
    };
    let mut cfg = render_cfg.clone();
    cfg.template =
        resolve_effective_template_for_single(cli.format, cfg.style, &lower);
    cfg.primary_source_name = Some(name);
    let budgets = make_budgets(cli, eff, eff_lines, eff_chars);
    if budgets.byte_budget.is_none()
        && budgets.char_budget.is_none()
        && budgets.line_budget.is_some()
    {
        cfg.string_free_prefix_graphemes = Some(40);
    }
    let out = match chosen_input {
        InputFormat::Json => {
            headson::headson_with_budgets(bytes, &cfg, &prio, budgets)?
        }
        InputFormat::Yaml => {
            headson::headson_yaml_with_budgets(bytes, &cfg, &prio, budgets)?
        }
        InputFormat::Text => {
            let is_code = headson::extensions::is_code_like_name(&lower);
            if is_code && matches!(cli.format, OutputFormat::Auto) {
                #[allow(
                    clippy::redundant_clone,
                    reason = "code branch requires its own config copy; other paths reuse the original"
                )]
                let mut cfg_code = cfg.clone();
                cfg_code.template = headson::OutputTemplate::Code;
                headson::headson_text_with_budgets_code(
                    bytes, &cfg_code, &prio, budgets,
                )?
            } else {
                headson::headson_text_with_budgets(
                    bytes, &cfg, &prio, budgets,
                )?
            }
        }
    };
    Ok((out, ignored))
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
    fn color_mode_from_flags(cli: &Cli) -> headson::ColorMode {
        if cli.color {
            headson::ColorMode::On
        } else if cli.no_color {
            headson::ColorMode::Off
        } else {
            headson::ColorMode::Auto
        }
    }

    // Select a baseline template; may be overridden per-input later.
    let template = match cli.format {
        OutputFormat::Auto => headson::OutputTemplate::Auto,
        OutputFormat::Json => {
            map_json_template_for_style(map_style(cli.style))
        }
        OutputFormat::Yaml => headson::OutputTemplate::Yaml,
        OutputFormat::Text => headson::OutputTemplate::Text,
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
    let color_mode = color_mode_from_flags(cli);
    let color_enabled = headson::resolve_color_enabled(color_mode);

    headson::RenderConfig {
        template,
        indent_unit,
        space,
        newline,
        prefer_tail_arrays: cli.tail,
        color_mode,
        color_enabled,
        style: map_style(cli.style),
        string_free_prefix_graphemes: None,
        debug: cli.debug,
        primary_source_name: None,
        show_fileset_headers: !cli.no_header,
    }
}

fn get_priority_config(
    per_file_budget: usize,
    cli: &Cli,
) -> headson::PriorityConfig {
    // Detect line-only mode: lines flag present and no explicit bytes/chars flags.
    let line_only = (cli.lines.is_some() || cli.global_lines.is_some())
        && cli.bytes.is_none()
        && cli.global_bytes.is_none();
    let array_max_items = if line_only {
        usize::MAX
    } else {
        (per_file_budget / 2).max(1)
    };
    headson::PriorityConfig {
        max_string_graphemes: cli.string_cap,
        array_max_items,
        prefer_tail_arrays: cli.tail,
        array_bias: headson::ArrayBias::HeadMidTail,
        array_sampler: if cli.tail {
            headson::ArraySamplerStrategy::Tail
        } else if cli.head {
            headson::ArraySamplerStrategy::Head
        } else {
            headson::ArraySamplerStrategy::Default
        },
        line_budget_only: line_only,
    }
}

fn map_style(s: StyleArg) -> headson::Style {
    match s {
        StyleArg::Strict => headson::Style::Strict,
        StyleArg::Default => headson::Style::Default,
        StyleArg::Detailed => headson::Style::Detailed,
    }
}

fn map_json_template_for_style(
    style: headson::Style,
) -> headson::OutputTemplate {
    match style {
        headson::Style::Strict => headson::OutputTemplate::Json,
        headson::Style::Default => headson::OutputTemplate::Pseudo,
        headson::Style::Detailed => headson::OutputTemplate::Js,
    }
}

fn resolve_effective_template_for_stdin(
    fmt: OutputFormat,
    style: headson::Style,
) -> headson::OutputTemplate {
    match fmt {
        OutputFormat::Auto | OutputFormat::Json => {
            map_json_template_for_style(style)
        }
        OutputFormat::Yaml => headson::OutputTemplate::Yaml,
        OutputFormat::Text => headson::OutputTemplate::Text,
    }
}

fn resolve_effective_template_for_single(
    fmt: OutputFormat,
    style: headson::Style,
    lower_name: &str,
) -> headson::OutputTemplate {
    match fmt {
        OutputFormat::Json => map_json_template_for_style(style),
        OutputFormat::Yaml => headson::OutputTemplate::Yaml,
        OutputFormat::Text => headson::OutputTemplate::Text,
        OutputFormat::Auto => {
            if lower_name.ends_with(".yaml") || lower_name.ends_with(".yml") {
                headson::OutputTemplate::Yaml
            } else if lower_name.ends_with(".json") {
                map_json_template_for_style(style)
            } else {
                // Unknown extension: prefer text template.
                headson::OutputTemplate::Text
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sorting::{FrecencyContext, sort_paths_with_context};
    use filetime::{FileTime, set_file_mtime};
    use git2::{Repository, Signature, Time};
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn rel_to_workdir(path: &std::path::Path, repo: &Repository) -> PathBuf {
        let workdir = repo.workdir().expect("workdir");
        let workdir_canon = workdir
            .canonicalize()
            .unwrap_or_else(|_| workdir.to_path_buf());
        let path_canon =
            path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        path.strip_prefix(workdir)
            .or_else(|_| path_canon.strip_prefix(&workdir_canon))
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                path.file_name()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| path.to_path_buf())
            })
    }

    fn commit_single_file(repo: &Repository, path: &std::path::Path) {
        let mut idx = repo.index().expect("index");
        let rel = rel_to_workdir(path, repo);
        idx.add_path(rel.as_path()).expect("add path");
        idx.write().expect("write index");
        let tree_id = idx.write_tree().expect("write tree");
        let tree = repo.find_tree(tree_id).expect("tree");
        let sig = Signature::new(
            "tester",
            "tester@example.com",
            &Time::new(1_700_000_000, 0),
        )
        .expect("sig");
        let parent = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .and_then(|oid| repo.find_commit(oid).ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &parents)
            .expect("commit");
    }

    #[test]
    fn sorts_by_mtime_when_no_frecency() {
        let temp = tempdir().expect("tempdir");
        let first = temp.path().join("first.txt");
        fs::write(&first, b"first").expect("write first");
        set_file_mtime(&first, FileTime::from_unix_time(1, 0))
            .expect("mtime first");
        let second = temp.path().join("second.txt");
        fs::write(&second, b"second").expect("write second");
        set_file_mtime(&second, FileTime::from_unix_time(2, 0))
            .expect("mtime second");

        let paths = vec![first.clone(), second.clone()];
        let sorted = sort_paths_with_context(&paths, temp.path(), None);
        assert_eq!(sorted, vec![second, first]);
    }

    #[test]
    fn prefers_frecency_rank_over_mtime() {
        let temp = tempdir().expect("tempdir");
        let newer = temp.path().join("newer.txt");
        let older = temp.path().join("older.txt");
        fs::write(&older, b"older").expect("write older");
        set_file_mtime(&older, FileTime::from_unix_time(5, 0))
            .expect("mtime older");
        fs::write(&newer, b"newer").expect("write newer");
        set_file_mtime(&newer, FileTime::from_unix_time(10, 0))
            .expect("mtime newer");

        let mut ctx = FrecencyContext {
            repo_root: temp
                .path()
                .canonicalize()
                .unwrap_or_else(|_| temp.path().to_path_buf()),
            ranks: HashMap::new(),
        };
        ctx.ranks.insert(OsString::from("older.txt"), 0);
        ctx.ranks.insert(OsString::from("newer.txt"), 1);

        let paths = vec![newer.clone(), older.clone()];
        let sorted = sort_paths_with_context(&paths, temp.path(), Some(&ctx));
        assert_eq!(sorted, vec![older, newer]);
    }

    #[test]
    fn frecency_context_builds_for_git_repo() {
        let dir = tempdir().expect("tempdir");
        let repo = Repository::init(dir.path()).expect("init repo");
        let file = dir.path().join("one.txt");
        fs::write(&file, b"hi").expect("write");
        commit_single_file(&repo, &file);

        unsafe {
            std::env::set_var("XDG_CACHE_HOME", dir.path().join("cache"));
        }
        let ctx = sorting::build_frecency_context(dir.path());
        assert!(
            ctx.as_ref()
                .and_then(|c| c.rank_for(std::path::Path::new("one.txt")))
                .is_some(),
            "frecency context should rank committed files"
        );
    }
}
