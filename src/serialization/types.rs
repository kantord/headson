#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OutputTemplate {
    Json,
    Pseudo,
    Js,
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
    // Desired color mode for rendering. Currently unused by templates,
    // but parsed and threaded through for future use.
    pub color_mode: ColorMode,
    // Resolved color enablement after considering color_mode and stdout TTY.
    pub color_enabled: bool,
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
