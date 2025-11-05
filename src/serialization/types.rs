#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OutputTemplate {
    Auto,
    Json,
    Pseudo,
    Js,
    Yaml,
    Text,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Style {
    Strict,
    Default,
    Detailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderConfig {
    pub template: OutputTemplate,
    pub indent_unit: String,
    pub space: String,
    // Newline sequence to use in final output (e.g., "\n" or "").
    // Templates read this directly; no post-processing replacement.
    pub newline: String,
    // When true, arrays prefer tail rendering (omission marker at start).
    pub prefer_tail_arrays: bool,
    // Desired color mode for rendering. Parsed and resolved to
    // `color_enabled`; templates receive color via the Out writer.
    pub color_mode: ColorMode,
    // Resolved color enablement after considering `color_mode` and stdout TTY.
    pub color_enabled: bool,
    // Output styling mode (controls omission annotations), orthogonal to template.
    pub style: Style,
    // When Some(n), and only a line budget is active, allow rendering up to
    // `n` graphemes of a string prefix regardless of top-K string-part inclusion.
    pub string_free_prefix_graphemes: Option<usize>,
    // When true, the core render path emits a debug JSON of the final
    // inclusion set to stderr before rendering. CLI sets this flag.
    pub debug: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ColorMode {
    On,
    Off,
    Auto,
}

impl ColorMode {
    // Returns whether coloring should be enabled given whether stdout is a TTY.
    pub fn effective(self, stdout_is_terminal: bool) -> bool {
        match self {
            ColorMode::On => true,
            ColorMode::Off => false,
            ColorMode::Auto => stdout_is_terminal,
        }
    }
}
