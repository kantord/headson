use std::path::PathBuf;

use clap::{ArgAction, Parser, ValueEnum};

/// Top-level CLI flags and enums.
#[derive(Parser, Debug)]
#[command(
    name = "hson",
    version,
    about = "Get a small but useful preview of JSON or YAML"
)]
pub struct Cli {
    #[arg(short = 'c', long = "bytes")]
    pub bytes: Option<usize>,
    #[arg(
        short = 'u',
        long = "chars",
        value_name = "CHARS",
        help = "Per-file Unicode character budget (adds up across files if no global chars limit)"
    )]
    pub chars: Option<usize>,
    #[arg(
        short = 'n',
        long = "lines",
        value_name = "LINES",
        help = "Per-file line budget (adds up across files if --global-lines not set). Fileset headers/summary lines do not consume this budget."
    )]
    pub lines: Option<usize>,
    #[arg(
        short = 'H',
        long = "count-headers",
        action = ArgAction::SetTrue,
        default_value_t = false,
        help = "Count fileset headers/summary lines toward budgets instead of treating them as free"
    )]
    pub count_headers: bool,
    #[arg(long = "no-space", default_value_t = false)]
    pub no_space: bool,
    #[arg(
        long = "no-newline",
        default_value_t = false,
        conflicts_with_all = ["lines", "global_lines"],
        help = "Do not add newlines in the output. Incompatible with --lines/--global-lines."
    )]
    pub no_newline: bool,
    #[arg(
        long = "no-header",
        default_value_t = false,
        help = "Suppress fileset section headers in the output"
    )]
    pub no_header: bool,
    #[arg(
        long = "no-sort",
        default_value_t = false,
        help = "Keep input order for filesets (skip frecency/mtime sorting)."
    )]
    pub no_sort: bool,
    #[arg(
        short = 'm',
        long = "compact",
        default_value_t = false,
        conflicts_with_all = ["no_space", "no_newline", "indent"],
        help = "Compact output with no added whitespace. Not very human-readable."
    )]
    pub compact: bool,
    #[arg(
        long = "string-cap",
        default_value_t = 500,
        help = "Maximum string length to display"
    )]
    pub string_cap: usize,
    #[arg(
        short = 'C',
        long = "global-bytes",
        value_name = "BYTES",
        help = "Total byte budget across all inputs. When combined with --bytes, the effective global limit is the smaller of the two."
    )]
    pub global_bytes: Option<usize>,
    #[arg(
        short = 'N',
        long = "global-lines",
        value_name = "LINES",
        help = "Total line budget across all inputs. Fileset headers/summary lines do not consume this budget."
    )]
    pub global_lines: Option<usize>,
    #[arg(
        long = "tail",
        default_value_t = false,
        help = "Prefer the end of arrays when truncating. Strings unaffected; JSON stays strict."
    )]
    pub tail: bool,
    #[arg(
        long = "head",
        default_value_t = false,
        conflicts_with = "tail",
        help = "Prefer the beginning of arrays when truncating (keep first N)."
    )]
    pub head: bool,
    #[arg(
        short = 'f',
        long = "format",
        value_enum,
        default_value_t = OutputFormat::Auto,
        help = "Output format: auto|json|yaml|text (filesets: auto is per-file)."
    )]
    pub format: OutputFormat,
    #[arg(
        short = 't',
        long = "template",
        value_enum,
        default_value_t = StyleArg::Default,
        help = "Output style: strict|default|detailed."
    )]
    pub style: StyleArg,
    #[arg(long = "indent", default_value = "  ")]
    pub indent: String,
    #[arg(
        long = "color",
        action = ArgAction::SetTrue,
        conflicts_with = "no_color",
        help = "Force enable ANSI colors in output"
    )]
    pub color: bool,
    #[arg(
        long = "no-color",
        action = ArgAction::SetTrue,
        conflicts_with = "color",
        help = "Disable ANSI colors in output"
    )]
    pub no_color: bool,
    #[arg(
        value_name = "INPUT",
        value_hint = clap::ValueHint::FilePath,
        num_args = 0..,
        help = "Optional file paths. If omitted, reads input from stdin. Multiple input files are supported. Directories and binary files are ignored with a notice on stderr."
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

pub fn get_render_config_from(cli: &Cli) -> headson::RenderConfig {
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
        count_fileset_headers_in_budgets: cli.count_headers,
    }
}

pub fn map_style(s: StyleArg) -> headson::Style {
    match s {
        StyleArg::Strict => headson::Style::Strict,
        StyleArg::Default => headson::Style::Default,
        StyleArg::Detailed => headson::Style::Detailed,
    }
}

pub fn map_json_template_for_style(
    style: headson::Style,
) -> headson::OutputTemplate {
    match style {
        headson::Style::Strict => headson::OutputTemplate::Json,
        headson::Style::Default => headson::OutputTemplate::Pseudo,
        headson::Style::Detailed => headson::OutputTemplate::Js,
    }
}
