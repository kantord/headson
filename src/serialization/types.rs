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
}
