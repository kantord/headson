use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use content_inspector::{ContentType, inspect};

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
    if cli.inputs.is_empty() {
        Ok((run_from_stdin(cli, &render_cfg)?, Vec::new()))
    } else {
        run_from_paths(cli, &render_cfg)
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
    let effective = budget::compute_effective(cli, input_count);
    let prio = budget::build_priority_config(cli, &effective);
    if cli.inputs.len() > 1 {
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
