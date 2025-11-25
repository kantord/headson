use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use content_inspector::{ContentType, inspect};
use ignore::{WalkBuilder, overrides::OverrideBuilder};

use crate::cli::args::{
    Cli, InputFormat, OutputFormat, get_render_config_from,
    map_json_template_for_style,
};
use crate::cli::budget;
use crate::sorting::sort_paths_for_fileset;

type InputEntry = (String, Vec<u8>);
type InputEntries = Vec<InputEntry>;
pub type IgnoreNotices = Vec<String>;

pub fn run(cli: &Cli) -> Result<(String, IgnoreNotices)> {
    let render_cfg = get_render_config_from(cli);
    let grep_cfg = crate::cli::args::build_grep_config(cli)?;
    let resolved_inputs = resolve_inputs(cli)?;
    if resolved_inputs.is_empty() {
        if !cli.globs.is_empty() {
            return Ok((
                String::new(),
                vec!["No files matched provided globs".to_string()],
            ));
        }
        Ok((run_from_stdin(cli, &render_cfg, &grep_cfg)?, Vec::new()))
    } else {
        run_from_paths(cli, &render_cfg, &grep_cfg, &resolved_inputs)
    }
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
    grep_cfg: &headson::GrepConfig,
) -> Result<String> {
    let input_bytes = read_stdin()?;
    let input_count = 1usize;
    let effective = budget::compute_effective(cli, input_count);
    let prio = budget::build_priority_config(cli, &effective);
    let mut cfg = render_cfg.clone();
    // Resolve effective output template for stdin:
    cfg.template = resolve_effective_template_for_stdin(cli.format, cfg.style);
    cfg = budget::render_config_for_budgets(cfg, &effective);
    let budgets = effective.budgets;
    let chosen_input = cli.input_format.unwrap_or(InputFormat::Json);
    let out = match chosen_input {
        InputFormat::Json => headson::headson(
            headson::InputKind::Json(input_bytes),
            &cfg,
            &prio,
            grep_cfg,
            budgets,
        )?,
        InputFormat::Yaml => headson::headson(
            headson::InputKind::Yaml(input_bytes),
            &cfg,
            &prio,
            grep_cfg,
            budgets,
        )?,
        InputFormat::Text => headson::headson(
            headson::InputKind::Text {
                bytes: input_bytes,
                mode: if matches!(cfg.template, headson::OutputTemplate::Code)
                {
                    headson::TextMode::CodeLike
                } else {
                    headson::TextMode::Plain
                },
            },
            &cfg,
            &prio,
            grep_cfg,
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
    grep_cfg: &headson::GrepConfig,
    inputs: &[PathBuf],
) -> Result<(String, IgnoreNotices)> {
    let sorted_inputs = if inputs.len() > 1 && !cli.no_sort {
        sort_paths_for_fileset(inputs)
    } else {
        inputs.to_vec()
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
    let effective = budget::compute_effective(cli, input_count);
    let prio = budget::build_priority_config(cli, &effective);
    if inputs.len() > 1 {
        if !matches!(cli.format, OutputFormat::Auto) {
            bail!(
                "--format cannot be customized for filesets; remove it or set to auto"
            );
        }
        let mut cfg = render_cfg.clone();
        // Filesets always render with per-file auto templates.
        cfg.template = headson::OutputTemplate::Auto;
        cfg = budget::render_config_for_budgets(cfg, &effective);
        let budgets = effective.budgets;
        let files: Vec<headson::FilesetInput> = entries
            .into_iter()
            .map(|(name, bytes)| {
                let kind = detect_fileset_input_kind(&name);
                headson::FilesetInput { name, bytes, kind }
            })
            .collect();
        let out = headson::headson(
            headson::InputKind::Fileset(files),
            &cfg,
            &prio,
            grep_cfg,
            budgets,
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
            if let Some(fmt) = cli.input_format {
                fmt
            } else if is_yaml_ext {
                InputFormat::Yaml
            } else if lower.ends_with(".json") {
                InputFormat::Json
            } else {
                InputFormat::Text
            }
        }
        OutputFormat::Json => cli.input_format.unwrap_or(InputFormat::Json),
        OutputFormat::Yaml => cli.input_format.unwrap_or(InputFormat::Yaml),
        OutputFormat::Text => cli.input_format.unwrap_or(InputFormat::Text),
    };
    let mut cfg = render_cfg.clone();
    cfg.template =
        resolve_effective_template_for_single(cli.format, cfg.style, &lower);
    cfg.primary_source_name = Some(name);
    cfg = budget::render_config_for_budgets(cfg, &effective);
    let budgets = effective.budgets;
    let out = match chosen_input {
        InputFormat::Json => headson::headson(
            headson::InputKind::Json(bytes),
            &cfg,
            &prio,
            grep_cfg,
            budgets,
        )?,
        InputFormat::Yaml => headson::headson(
            headson::InputKind::Yaml(bytes),
            &cfg,
            &prio,
            grep_cfg,
            budgets,
        )?,
        InputFormat::Text => {
            let is_code = headson::extensions::is_code_like_name(&lower);
            if is_code && matches!(cli.format, OutputFormat::Auto) {
                #[allow(
                    clippy::redundant_clone,
                    reason = "code branch requires its own config copy; other paths reuse the original"
                )]
                let mut cfg_code = cfg.clone();
                cfg_code.template = headson::OutputTemplate::Code;
                headson::headson(
                    headson::InputKind::Text {
                        bytes,
                        mode: headson::TextMode::CodeLike,
                    },
                    &cfg_code,
                    &prio,
                    grep_cfg,
                    budgets,
                )?
            } else {
                headson::headson(
                    headson::InputKind::Text {
                        bytes,
                        mode: if matches!(
                            cfg.template,
                            headson::OutputTemplate::Code
                        ) {
                            headson::TextMode::CodeLike
                        } else {
                            headson::TextMode::Plain
                        },
                    },
                    &cfg,
                    &prio,
                    grep_cfg,
                    budgets,
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

fn resolve_inputs(cli: &Cli) -> Result<Vec<PathBuf>> {
    let cwd =
        env::current_dir().context("failed to read current directory")?;
    let mut seen_abs: HashSet<PathBuf> = HashSet::new();
    let mut inputs: Vec<PathBuf> = Vec::new();

    for path in &cli.inputs {
        push_unique(&cwd, &mut seen_abs, &mut inputs, path);
    }

    if !cli.globs.is_empty() {
        let gitignore = load_gitignore(&cwd);
        collect_glob_matches(
            &cli.globs,
            &cwd,
            &mut seen_abs,
            &mut inputs,
            gitignore.as_ref(),
            cli.no_sort,
        )?;
    }

    Ok(inputs)
}

fn push_unique(
    cwd: &Path,
    seen_abs: &mut HashSet<PathBuf>,
    inputs: &mut Vec<PathBuf>,
    path: &Path,
) {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    if seen_abs.insert(abs) {
        inputs.push(path.to_path_buf());
    }
}

fn relativize<'a>(path: &'a Path, cwd: &Path) -> &'a Path {
    path.strip_prefix(cwd)
        .or_else(|_| path.strip_prefix("."))
        .unwrap_or(path)
}

fn load_gitignore(cwd: &Path) -> Option<ignore::gitignore::Gitignore> {
    let gi_path = cwd.join(".gitignore");
    let (gi, err) = ignore::gitignore::Gitignore::new(gi_path);
    if err.is_none() { Some(gi) } else { None }
}

fn collect_glob_matches(
    patterns: &[String],
    cwd: &Path,
    seen_abs: &mut HashSet<PathBuf>,
    inputs: &mut Vec<PathBuf>,
    gitignore: Option<&ignore::gitignore::Gitignore>,
    no_sort: bool,
) -> Result<()> {
    if no_sort {
        // Expand each glob in the order provided so --no-sort preserves user intent.
        for pattern in patterns {
            let mut overrides = OverrideBuilder::new(".");
            overrides
                .add(pattern)
                .with_context(|| format!("invalid glob pattern: {pattern}"))?;
            let overrides = overrides
                .build()
                .context("failed to compile glob overrides")?;
            let mut walker = WalkBuilder::new(".");
            // Still sort within each glob for deterministic traversal.
            configure_walker(&mut walker, overrides, true);
            collect_from_walker(&walker, cwd, seen_abs, inputs, gitignore)?;
        }
        return Ok(());
    }

    let mut overrides = OverrideBuilder::new(".");
    for pattern in patterns {
        overrides
            .add(pattern)
            .with_context(|| format!("invalid glob pattern: {pattern}"))?;
    }
    let overrides = overrides
        .build()
        .context("failed to compile glob overrides")?;

    let mut walker = WalkBuilder::new(".");
    configure_walker(&mut walker, overrides, true);
    collect_from_walker(&walker, cwd, seen_abs, inputs, gitignore)?;
    Ok(())
}

fn configure_walker(
    walker: &mut WalkBuilder,
    overrides: ignore::overrides::Override,
    should_sort: bool,
) {
    walker.overrides(overrides);
    walker.git_ignore(true);
    walker.git_global(true);
    walker.git_exclude(true);
    walker.require_git(false);
    walker.add_custom_ignore_filename(".gitignore");
    if should_sort {
        // Deterministic expansion keeps traversal stable; fileset ordering is still
        // resolved later (mtime/frecency or --no-sort) on the collected list.
        walker.sort_by_file_name(std::cmp::Ord::cmp);
    } else {
        // Keep discovery order stable for --no-sort: single-threaded walk and no sorting.
        walker.threads(1);
        walker.sort_by_file_name(|_, _| std::cmp::Ordering::Equal);
    }
}

fn collect_from_walker(
    walker: &WalkBuilder,
    cwd: &Path,
    seen_abs: &mut HashSet<PathBuf>,
    inputs: &mut Vec<PathBuf>,
    gitignore: Option<&ignore::gitignore::Gitignore>,
) -> Result<()> {
    for dent in walker.build() {
        let dir_entry = dent?;
        if !dir_entry
            .file_type()
            .map(|ft| ft.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let path = dir_entry.into_path();
        let rel = relativize(&path, cwd).to_path_buf();
        if gitignore.is_some_and(|gi| {
            gi.matched_path_or_any_parents(&rel, false).is_ignore()
        }) {
            continue;
        }
        push_unique(cwd, seen_abs, inputs, &rel);
    }
    Ok(())
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
    use crate::cli::args::Cli;
    use clap::Parser;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn explicit_input_format_overrides_auto_detection_for_single_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("object.json");
        fs::write(&path, "not json\nline2\n").unwrap();

        let cli =
            Cli::parse_from(["hson", "-i", "text", path.to_str().unwrap()]);

        let (out, notices) = run(&cli).expect("run succeeds with text ingest");
        assert!(notices.is_empty());
        assert!(
            out.contains("not json"),
            "should treat .json as text when -i text is passed"
        );
    }

    #[test]
    fn auto_detection_still_applies_when_no_input_flag() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("object.json");
        fs::write(&path, "{\"a\":1}").unwrap();

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);

        let (out, notices) =
            run(&cli).expect("run succeeds with default ingest");
        assert!(notices.is_empty());
        assert!(
            out.contains("\"a\"") || out.contains("a"),
            "auto mode should still treat .json as json when -i is absent"
        );
    }
}
