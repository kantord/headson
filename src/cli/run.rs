use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use content_inspector::{ContentType, inspect};
use ignore::WalkBuilder;
use ignore::overrides::{Override, OverrideBuilder};

use crate::cli::args::{
    Cli, InputFormat, OutputFormat, get_render_config_from,
};
use crate::cli::budget;
use crate::sorting::sort_paths_for_fileset;

type InputEntry = (String, Vec<u8>);
type InputEntries = Vec<InputEntry>;
pub(crate) type CliWarnings = Vec<String>;

fn build_effective_configs(
    cli: &Cli,
    mut render_cfg: headson::RenderConfig,
    input_count: usize,
) -> (
    headson::RenderConfig,
    headson::PriorityConfig,
    headson::Budgets,
) {
    let effective = budget::compute_effective(cli, input_count);
    let prio = budget::build_priority_config(cli, &effective);
    render_cfg = budget::render_config_for_budgets(render_cfg, &effective);
    (render_cfg, prio, effective.budgets)
}

fn needs_fileset(cli: &Cli, inputs_len: usize) -> bool {
    inputs_len > 1 || cli.tree
}

pub(crate) fn run(cli: &Cli) -> Result<(String, CliWarnings)> {
    budget::validate(cli)?;
    let mut render_cfg = get_render_config_from(cli);
    let grep_cfg = headson::build_grep_config(
        cli.grep.as_deref(),
        cli.weak_grep.as_deref(),
        crate::cli::args::map_grep_show(cli.grep_show),
    )?;
    render_cfg.grep_highlight = grep_cfg.regex.clone();
    let resolved_inputs = resolve_inputs(cli)?;
    if resolved_inputs.is_empty() {
        if !cli.globs.is_empty() || cli.recursive {
            return Ok((
                String::new(),
                vec!["No files matched provided inputs".to_string()],
            ));
        }
        if cli.tree {
            bail!("--tree requires file inputs; stdin mode is not supported");
        }
        Ok(run_from_stdin(cli, &render_cfg, &grep_cfg)?)
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
        fileset_text_kind(&lower)
    }
}

fn fileset_text_kind(name: &str) -> headson::FilesetInputKind {
    let atomic = headson::extensions::is_code_like_name(name);
    headson::FilesetInputKind::Text {
        atomic_lines: atomic,
    }
}

fn run_from_stdin(
    cli: &Cli,
    render_cfg: &headson::RenderConfig,
    grep_cfg: &headson::GrepConfig,
) -> Result<(String, CliWarnings)> {
    let input_bytes = read_stdin()?;
    let input_count = 1usize;
    let mut cfg = render_cfg.clone();
    cfg.template = resolve_effective_template_for_stdin(cli.format, cfg.style);
    let (cfg, prio, budgets) = build_effective_configs(cli, cfg, input_count);
    let chosen_input = cli.input_format.unwrap_or(InputFormat::Json);
    let (out, warnings) = render_single_input(
        chosen_input,
        input_bytes,
        &cfg,
        &prio,
        grep_cfg,
        budgets,
    )?;
    Ok((out, warnings))
}

fn run_from_paths(
    cli: &Cli,
    render_cfg: &headson::RenderConfig,
    grep_cfg: &headson::GrepConfig,
    inputs: &[PathBuf],
) -> Result<(String, CliWarnings)> {
    let sorted_inputs = if needs_fileset(cli, inputs.len()) && !cli.no_sort {
        sort_paths_for_fileset(inputs)
    } else {
        inputs.to_vec()
    };
    if std::env::var_os("HEADSON_FRECEN_TRACE").is_some() {
        eprintln!("run_from_paths sorted_inputs={sorted_inputs:?}");
    }
    let (entries, warnings) = ingest_paths(&sorted_inputs)?;
    if std::env::var_os("HEADSON_FRECEN_TRACE").is_some() {
        eprintln!(
            "run_from_paths ingested={:?}",
            entries.iter().map(|(n, _)| n).collect::<Vec<_>>()
        );
    }
    if needs_fileset(cli, inputs.len()) {
        return render_fileset(entries, warnings, cli, render_cfg, grep_cfg);
    }
    if entries.is_empty() {
        return Ok((String::new(), warnings));
    }
    render_single_entry(entries, warnings, cli, render_cfg, grep_cfg)
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

fn ingest_paths(paths: &[PathBuf]) -> Result<(InputEntries, CliWarnings)> {
    let mut out: InputEntries = Vec::with_capacity(paths.len());
    let mut warnings: CliWarnings = Vec::new();
    for path in paths.iter() {
        let display = path.display().to_string();
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.is_dir() {
                warnings.push(format!("Ignored directory: {display}"));
                continue;
            }
        }
        if let Some(bytes) = sniff_then_read_text(path)? {
            out.push((display, bytes))
        } else {
            warnings.push(format!("Ignored binary file: {display}"));
            continue;
        }
    }
    Ok((out, warnings))
}

fn resolve_inputs(cli: &Cli) -> Result<Vec<PathBuf>> {
    let cwd =
        env::current_dir().context("failed to read current directory")?;
    let mut collector = InputCollector::new(&cwd);
    if cli.recursive && cli.inputs.is_empty() {
        bail!(
            "--recursive requires directory inputs; stdin mode is not supported"
        );
    }

    if cli.recursive {
        for path in &cli.inputs {
            collector.expand_recursive_dir(path, cli.no_sort)?;
        }
    } else {
        for path in &cli.inputs {
            collector.add_explicit(path);
        }
    }

    if !cli.globs.is_empty() {
        collector.expand_globs(&cli.globs, cli.no_sort)?;
    }

    Ok(collector.finish())
}

struct InputCollector {
    display_root: PathBuf,
    seen_abs: HashSet<PathBuf>,
    inputs: Vec<PathBuf>,
}

impl InputCollector {
    fn new(display_root: &Path) -> Self {
        Self {
            display_root: display_root.to_path_buf(),
            seen_abs: HashSet::new(),
            inputs: Vec::new(),
        }
    }

    fn finish(self) -> Vec<PathBuf> {
        self.inputs
    }

    fn add_explicit(&mut self, path: &Path) {
        add_simple_input(
            &self.display_root,
            &mut self.seen_abs,
            &mut self.inputs,
            path,
        );
    }

    fn expand_recursive_dir(
        &mut self,
        path: &Path,
        no_sort: bool,
    ) -> Result<()> {
        let dir = ensure_recursive_dir(&self.display_root, path)?;
        let dir_norm = normalize_path(&dir);
        self.expand_globs_in_root(&dir_norm, &["**/*".to_string()], no_sort)
    }

    fn expand_globs(
        &mut self,
        patterns: &[String],
        no_sort: bool,
    ) -> Result<()> {
        let root = self.display_root.clone();
        self.expand_globs_in_root(&root, patterns, no_sort)
    }

    fn expand_globs_in_root(
        &mut self,
        root: &Path,
        patterns: &[String],
        no_sort: bool,
    ) -> Result<()> {
        if no_sort {
            // Expand each glob in the order provided so --no-sort preserves user intent.
            for pattern in patterns {
                let overrides = build_override_matcher(
                    root,
                    std::iter::once(pattern.as_str()),
                )?;
                let mut walker = WalkBuilder::new(root);
                configure_input_walker(&mut walker, false);
                collect_from_walker(
                    &walker,
                    &self.display_root,
                    root,
                    &mut self.seen_abs,
                    &mut self.inputs,
                    Some(&overrides),
                )?;
            }
            return Ok(());
        }

        let overrides =
            build_override_matcher(root, patterns.iter().map(String::as_str))?;
        let mut walker = WalkBuilder::new(root);
        configure_input_walker(&mut walker, true);
        collect_from_walker(
            &walker,
            &self.display_root,
            root,
            &mut self.seen_abs,
            &mut self.inputs,
            Some(&overrides),
        )?;
        Ok(())
    }
}

fn add_simple_input(
    cwd: &Path,
    seen_abs: &mut HashSet<PathBuf>,
    inputs: &mut Vec<PathBuf>,
    path: &Path,
) -> bool {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    if seen_abs.insert(abs) {
        inputs.push(path.to_path_buf());
        true
    } else {
        false
    }
}

fn display_relative_path<'a>(path: &'a Path, display_root: &Path) -> &'a Path {
    path.strip_prefix(display_root)
        .or_else(|_| path.strip_prefix("."))
        .unwrap_or(path)
}

fn configure_input_walker(walker: &mut WalkBuilder, should_sort: bool) {
    walker.ignore(true);
    walker.git_ignore(true);
    walker.git_global(true);
    walker.git_exclude(true);
    walker.require_git(false);
    walker.add_custom_ignore_filename(".rgignore");
    if should_sort {
        // Deterministic expansion keeps traversal stable; fileset ordering is still
        // resolved later (mtime/frecency or --no-sort) on the collected list.
        walker.sort_by_file_name(std::cmp::Ord::cmp);
    } else {
        // Keep discovery order stable for --no-sort: single-threaded walk.
        walker.threads(1);
    }
}

fn ensure_recursive_dir(cwd: &Path, path: &Path) -> Result<PathBuf> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let meta = std::fs::metadata(&abs).with_context(|| {
        format!("failed to read input path: {}", path.display())
    })?;
    if !meta.is_dir() {
        bail!(
            "--recursive requires directory inputs (got file: {})",
            path.display()
        );
    }
    Ok(abs)
}

fn collect_from_walker(
    walker: &WalkBuilder,
    display_root: &Path,
    override_root: &Path,
    seen_abs: &mut HashSet<PathBuf>,
    inputs: &mut Vec<PathBuf>,
    matcher: Option<&Override>,
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
        let rel = display_relative_path(&path, display_root).to_path_buf();
        if let Some(matcher) = matcher {
            let match_path =
                path.strip_prefix(override_root).unwrap_or(path.as_path());
            let match_result = matcher.matched(match_path, false);
            if !match_result.is_whitelist() {
                continue;
            }
        }
        add_simple_input(display_root, seen_abs, inputs, &rel);
    }
    Ok(())
}

fn build_override_matcher<'a, I>(root: &Path, patterns: I) -> Result<Override>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut builder = OverrideBuilder::new(root);
    for pattern in patterns {
        if pattern.starts_with('!') {
            bail!(
                "negated glob patterns are not supported; use ignore files instead: {pattern}"
            );
        }
        builder
            .add(pattern)
            .with_context(|| format!("invalid glob pattern: {pattern}"))?;
    }
    builder.build().context("failed to compile glob overrides")
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let mut has_root = false;
    for comp in path.components() {
        match comp {
            std::path::Component::Prefix(prefix) => {
                out.push(prefix.as_os_str());
            }
            std::path::Component::RootDir => {
                out.push(comp.as_os_str());
                has_root = true;
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() && !has_root {
                    out.push(comp.as_os_str());
                }
            }
            std::path::Component::Normal(part) => out.push(part),
        }
    }
    out
}

fn render_single_input(
    input_format: InputFormat,
    bytes: Vec<u8>,
    cfg: &headson::RenderConfig,
    prio: &headson::PriorityConfig,
    grep_cfg: &headson::GrepConfig,
    budgets: headson::Budgets,
) -> Result<(String, CliWarnings)> {
    let text_mode = if matches!(cfg.template, headson::OutputTemplate::Code) {
        headson::TextMode::CodeLike
    } else {
        headson::TextMode::Plain
    };
    match input_format {
        InputFormat::Json => headson::headson(
            headson::InputKind::Json(bytes),
            cfg,
            prio,
            grep_cfg,
            budgets,
        ),
        InputFormat::Yaml => headson::headson(
            headson::InputKind::Yaml(bytes),
            cfg,
            prio,
            grep_cfg,
            budgets,
        ),
        InputFormat::Text => headson::headson(
            headson::InputKind::Text {
                bytes,
                mode: text_mode,
            },
            cfg,
            prio,
            grep_cfg,
            budgets,
        ),
    }
    .map(|out| (out.text, out.warnings))
}

fn resolve_effective_template_for_stdin(
    fmt: OutputFormat,
    style: headson::Style,
) -> headson::OutputTemplate {
    match fmt {
        OutputFormat::Auto | OutputFormat::Json => {
            headson::map_json_template_for_style(style)
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
        OutputFormat::Json => headson::map_json_template_for_style(style),
        OutputFormat::Yaml => headson::OutputTemplate::Yaml,
        OutputFormat::Text => headson::OutputTemplate::Text,
        OutputFormat::Auto => {
            if lower_name.ends_with(".yaml") || lower_name.ends_with(".yml") {
                headson::OutputTemplate::Yaml
            } else if lower_name.ends_with(".json") {
                headson::map_json_template_for_style(style)
            } else {
                // Unknown extension: prefer text template.
                headson::OutputTemplate::Text
            }
        }
    }
}

fn render_fileset(
    entries: InputEntries,
    mut warnings: CliWarnings,
    cli: &Cli,
    render_cfg: &headson::RenderConfig,
    grep_cfg: &headson::GrepConfig,
) -> Result<(String, CliWarnings)> {
    if !matches!(cli.format, OutputFormat::Auto) {
        bail!(
            "--format cannot be customized for filesets; remove it or set to auto"
        );
    }
    let mut cfg = render_cfg.clone();
    cfg.template = headson::OutputTemplate::Auto;
    let input_count = entries.len().max(1);
    let (cfg, prio, budgets) = build_effective_configs(cli, cfg, input_count);
    let files: Vec<headson::FilesetInput> = entries
        .into_iter()
        .map(|(name, bytes)| {
            let kind = detect_fileset_input_kind(&name);
            headson::FilesetInput { name, bytes, kind }
        })
        .collect();
    let headson::RenderOutput {
        text: out,
        warnings: fallback_warnings,
    } = headson::headson(
        headson::InputKind::Fileset(files),
        &cfg,
        &prio,
        grep_cfg,
        budgets,
    )?;
    warnings.extend(fallback_warnings);
    if grep_cfg.regex.is_some()
        && matches!(grep_cfg.show, headson::GrepShow::Matching)
        && !grep_cfg.weak
        && out.trim().is_empty()
    {
        warnings.push("No grep matches found".to_string());
    }
    Ok((out, warnings))
}

fn render_single_entry(
    mut entries: InputEntries,
    mut warnings: CliWarnings,
    cli: &Cli,
    render_cfg: &headson::RenderConfig,
    grep_cfg: &headson::GrepConfig,
) -> Result<(String, CliWarnings)> {
    let (name, bytes) = entries
        .pop()
        .expect("single-entry render expects one ingested input");
    let lower = name.to_ascii_lowercase();
    let chosen_input = select_input_format(cli, &lower);
    let cfg_for_render = build_single_render_config(
        cli,
        render_cfg,
        &lower,
        &name,
        chosen_input,
    );
    let (cfg_for_render, prio, budgets) =
        build_effective_configs(cli, cfg_for_render, 1usize);
    let (out, mut fallback_warnings) = render_single_input(
        chosen_input,
        bytes,
        &cfg_for_render,
        &prio,
        grep_cfg,
        budgets,
    )
    .with_context(|| format!("failed to parse input file: {name}"))?;
    if !fallback_warnings.is_empty() {
        warnings.append(&mut fallback_warnings);
    }
    Ok((out, warnings))
}

fn build_single_render_config(
    cli: &Cli,
    render_cfg: &headson::RenderConfig,
    lower_name: &str,
    source_name: &str,
    chosen_input: InputFormat,
) -> headson::RenderConfig {
    let mut cfg = render_cfg.clone();
    cfg.template = resolve_effective_template_for_single(
        cli.format, cfg.style, lower_name,
    );
    cfg.primary_source_name = Some(source_name.to_string());
    if let InputFormat::Text = chosen_input {
        let is_auto = matches!(cli.format, OutputFormat::Auto);
        let is_code = headson::extensions::is_code_like_name(
            cfg.primary_source_name.as_deref().unwrap_or_default(),
        );
        if is_auto
            && is_code
            && matches!(cfg.template, headson::OutputTemplate::Text)
        {
            cfg.template = headson::OutputTemplate::Code;
        }
    }
    cfg
}

fn select_input_format(cli: &Cli, lower_name: &str) -> InputFormat {
    let is_yaml_ext =
        lower_name.ends_with(".yaml") || lower_name.ends_with(".yml");
    match cli.format {
        OutputFormat::Auto => {
            if let Some(fmt) = cli.input_format {
                fmt
            } else if is_yaml_ext {
                InputFormat::Yaml
            } else if lower_name.ends_with(".json") {
                InputFormat::Json
            } else {
                InputFormat::Text
            }
        }
        OutputFormat::Json => cli.input_format.unwrap_or(InputFormat::Json),
        OutputFormat::Yaml => cli.input_format.unwrap_or(InputFormat::Yaml),
        OutputFormat::Text => cli.input_format.unwrap_or(InputFormat::Text),
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

        let (out, warnings) =
            run(&cli).expect("run succeeds with text ingest");
        assert!(warnings.is_empty());
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

        let (out, warnings) =
            run(&cli).expect("run succeeds with default ingest");
        assert!(warnings.is_empty());
        assert!(
            out.contains("\"a\"") || out.contains("a"),
            "auto mode should still treat .json as json when -i is absent"
        );
    }
}
