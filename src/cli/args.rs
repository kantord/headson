use std::path::PathBuf;

use clap::{ArgAction, Parser, ValueEnum};
use clap_complete::Shell;

/// Top-level CLI flags and enums.
#[derive(Parser, Debug)]
#[command(
    name = "hson",
    version,
    about = "Get a small but useful preview of JSON or YAML"
)]
#[clap(group = clap::ArgGroup::new("strong_grep").args(["grep", "igrep"]).multiple(true))]
#[clap(group = clap::ArgGroup::new("weak_grep_group").args(["weak_grep", "weak_igrep"]).multiple(true))]
pub struct Cli {
    #[arg(short = 'c', long = "bytes", help_heading = "Preview Size")]
    pub bytes: Option<usize>,
    #[arg(
        short = 'u',
        long = "chars",
        value_name = "CHARS",
        help = "Per-file Unicode character budget (adds up across files if no global chars limit)",
        help_heading = "Preview Size"
    )]
    pub chars: Option<usize>,
    #[arg(
        short = 'n',
        long = "lines",
        value_name = "LINES",
        help = "Per-file line budget. Pass --global-lines to also cap the total across inputs. Fileset headers/summary lines do not consume this budget.",
        help_heading = "Preview Size"
    )]
    pub lines: Option<usize>,
    #[arg(
        short = 'H',
        long = "count-headers",
        action = ArgAction::SetTrue,
        default_value_t = false,
        help = "Count fileset headers/summary lines toward budgets instead of treating them as free",
        help_heading = "Preview Size"
    )]
    pub count_headers: bool,
    #[arg(
        long = "no-space",
        default_value_t = false,
        help_heading = "Output Format"
    )]
    pub no_space: bool,
    #[arg(
        long = "no-newline",
        default_value_t = false,
        conflicts_with_all = ["lines", "global_lines"],
        help = "Do not add newlines in the output. Incompatible with --lines/--global-lines.",
        help_heading = "Output Format"
    )]
    pub no_newline: bool,
    #[arg(
        long = "no-header",
        default_value_t = false,
        help = "Suppress fileset section headers in the output",
        help_heading = "Multi-file Mode"
    )]
    pub no_header: bool,
    #[arg(
        long = "tree",
        default_value_t = false,
        conflicts_with_all = ["no_header", "compact", "no_newline"],
        help = "Render filesets in a directory tree layout with inline previews",
        help_heading = "Multi-file Mode"
    )]
    pub tree: bool,
    #[arg(
        long = "no-sort",
        default_value_t = false,
        help = "Keep input order for filesets (skip frecency/mtime sorting).",
        help_heading = "Multi-file Mode"
    )]
    pub no_sort: bool,
    #[arg(
        short = 'm',
        long = "compact",
        default_value_t = false,
        conflicts_with_all = ["no_space", "no_newline", "indent"],
        help = "Compact output with no added whitespace. Not very human-readable.",
        help_heading = "Output Format"
    )]
    pub compact: bool,
    #[arg(
        long = "string-cap",
        default_value_t = 500,
        help = "Maximum string length to display",
        help_heading = "Preview Size"
    )]
    pub string_cap: usize,
    #[arg(
        short = 'C',
        long = "global-bytes",
        value_name = "BYTES",
        help = "Total byte budget across all inputs. When combined with --bytes, the effective global limit is the smaller of the two.",
        help_heading = "Preview Size"
    )]
    pub global_bytes: Option<usize>,
    #[arg(
        short = 'N',
        long = "global-lines",
        value_name = "LINES",
        help = "Total line budget across all inputs. Fileset headers/summary lines do not consume this budget.",
        help_heading = "Preview Size"
    )]
    pub global_lines: Option<usize>,
    #[arg(
        long = "tail",
        default_value_t = false,
        help = "Prefer the end of arrays when truncating. Strings unaffected; JSON stays strict.",
        help_heading = "Preview Size"
    )]
    pub tail: bool,
    #[arg(
        long = "head",
        default_value_t = false,
        conflicts_with = "tail",
        help = "Prefer the beginning of arrays when truncating (keep first N).",
        help_heading = "Preview Size"
    )]
    pub head: bool,
    #[arg(
        short = 'f',
        long = "format",
        value_enum,
        default_value_t = OutputFormat::Auto,
        help = "Output format: auto|json|yaml|text (filesets: auto is per-file).",
        help_heading = "Output Format"
    )]
    pub format: OutputFormat,
    #[arg(
        short = 't',
        long = "template",
        value_enum,
        default_value_t = StyleArg::Default,
        help = "Output style: strict|default|detailed.",
        help_heading = "Output Format"
    )]
    pub style: StyleArg,
    #[arg(
        long = "indent",
        default_value = "  ",
        help_heading = "Output Format"
    )]
    pub indent: String,
    #[arg(
        long = "color",
        action = ArgAction::SetTrue,
        conflicts_with = "no_color",
        help = "Force enable ANSI colors in output",
        help_heading = "Output Format"
    )]
    pub color: bool,
    #[arg(
        long = "no-color",
        action = ArgAction::SetTrue,
        conflicts_with = "color",
        help = "Disable ANSI colors in output",
        help_heading = "Output Format"
    )]
    pub no_color: bool,
    #[arg(
        short = 'g',
        long = "glob",
        value_name = "PATTERN",
        num_args = 0..,
        help = "Additional input glob(s) to expand (respects .gitignore). Can be used multiple times.",
        help_heading = "Multi-file Mode"
    )]
    pub globs: Vec<String>,
    #[arg(
        short = 'r',
        long = "recursive",
        action = ArgAction::SetTrue,
        conflicts_with = "globs",
        help = "Recursively expand directory inputs (like grep -r). Requires directory paths.",
        help_heading = "Multi-file Mode"
    )]
    pub recursive: bool,
    #[arg(
        value_name = "INPUT",
        value_hint = clap::ValueHint::FilePath,
        num_args = 0..,
        help = "Optional file paths. If omitted, reads input from stdin. Multiple input files are supported. Directories are ignored unless --recursive is set; binary files are ignored with a warning on stderr."
    )]
    pub inputs: Vec<PathBuf>,
    #[arg(
        short = 'i',
        long = "input-format",
        value_enum,
        help = "Input ingestion format: json|yaml|text. Default is json for stdin/filesets; auto-detected for single-file auto runs."
    )]
    pub input_format: Option<InputFormat>,
    #[arg(
        long = "debug",
        default_value_t = false,
        help = "Dump pruned internal tree (JSON) to stderr for the final render attempt"
    )]
    pub debug: bool,
    #[arg(
        long = "grep",
        value_name = "REGEX",
        action = ArgAction::Append,
        help = "Guarantee inclusion of values (and their ancestors) matching this regex; budgets apply to everything else. Repeatable; multiple patterns match with OR.",
        help_heading = "Filtering"
    )]
    pub grep: Vec<String>,
    #[arg(
        long = "igrep",
        value_name = "REGEX",
        action = ArgAction::Append,
        help = "Case-insensitive --grep. Repeatable and combinable with --grep (OR).",
        help_heading = "Filtering"
    )]
    pub igrep: Vec<String>,
    #[arg(
        long = "weak-grep",
        value_name = "REGEX",
        action = ArgAction::Append,
        help = "Bias priority toward matches without guaranteeing inclusion. Repeatable; multiple patterns match with OR. Can combine with --grep/--igrep.",
        help_heading = "Filtering"
    )]
    pub weak_grep: Vec<String>,
    #[arg(
        long = "weak-igrep",
        value_name = "REGEX",
        action = ArgAction::Append,
        help = "Case-insensitive --weak-grep. Repeatable and combinable with --weak-grep (OR). Can combine with --grep/--igrep.",
        help_heading = "Filtering"
    )]
    pub weak_igrep: Vec<String>,
    #[arg(
        long = "grep-show",
        value_enum,
        default_value_t = GrepShowArg::Matching,
        requires = "strong_grep",
        help = "When using --grep or --igrep, control fileset inclusion: matching (default) | all",
        help_heading = "Filtering"
    )]
    pub grep_show: GrepShowArg,
    #[arg(
        long = "completions",
        value_name = "SHELL",
        value_enum,
        help = "Print shell completions for the given shell"
    )]
    pub completions: Option<Shell>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Auto,
    Json,
    Yaml,
    Text,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum StyleArg {
    Strict,
    Default,
    Detailed,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum InputFormat {
    Json,
    Yaml,
    Text,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum GrepShowArg {
    Matching,
    All,
}

pub fn get_render_config_from(cli: &Cli) -> headson::RenderConfig {
    let template = base_template(cli);
    let (indent_unit, space, newline) = whitespace_from(cli);
    let color_mode = color_mode_from_flags(cli);
    let color_enabled = headson::resolve_color_enabled(color_mode);
    let (show_fileset_headers, fileset_tree, count_fileset_headers_in_budgets) =
        fileset_flags(cli);
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
        show_fileset_headers,
        fileset_tree,
        count_fileset_headers_in_budgets,
        grep_highlight: None,
    }
}

fn base_template(cli: &Cli) -> headson::OutputTemplate {
    match cli.format {
        OutputFormat::Auto => headson::OutputTemplate::Auto,
        OutputFormat::Json => {
            headson::map_json_template_for_style(map_style(cli.style))
        }
        OutputFormat::Yaml => headson::OutputTemplate::Yaml,
        OutputFormat::Text => headson::OutputTemplate::Text,
    }
}

fn whitespace_from(cli: &Cli) -> (String, String, String) {
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
    (indent_unit, space, newline)
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

fn fileset_flags(cli: &Cli) -> (bool, bool, bool) {
    // In tree mode show_fileset_headers controls whether scaffolding counts toward budgets;
    // CLI already forbids --tree with --no-header.
    (!cli.no_header, cli.tree, cli.count_headers)
}

pub fn map_style(s: StyleArg) -> headson::Style {
    match s {
        StyleArg::Strict => headson::Style::Strict,
        StyleArg::Default => headson::Style::Default,
        StyleArg::Detailed => headson::Style::Detailed,
    }
}

pub(crate) fn map_grep_show(show: GrepShowArg) -> headson::GrepShow {
    match show {
        GrepShowArg::Matching => headson::GrepShow::Matching,
        GrepShowArg::All => headson::GrepShow::All,
    }
}

/// See also
/// <https://github.com/clap-rs/clap/blob/f65d421607ba16c3175ffe76a20820f123b6c4cb/clap_complete/examples/completion-derive.rs#L69>.
pub fn print_completions<G: clap_complete::Generator>(
    generator: G,
    cmd: &mut clap::Command,
) {
    clap_complete::generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut std::io::stdout(),
    );
}
