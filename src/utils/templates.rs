use crate::serialization::types::{OutputTemplate, Style};

/// Map a style to its corresponding JSON-family output template.
pub fn map_json_template_for_style(style: Style) -> OutputTemplate {
    match style {
        Style::Strict => OutputTemplate::Json,
        Style::Default => OutputTemplate::Pseudo,
        Style::Detailed => OutputTemplate::Js,
    }
}
