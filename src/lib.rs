use anyhow::Result;
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

pub fn parse_json(input: &str, budget: usize) -> Result<Value> {
    let parsed_value: Value = serde_json::from_str(input)?;

    if let Value::Array(elements) = &parsed_value {
        if elements.len() == 1 {
            if let Some(Value::String(s)) = elements.get(0) {
                let length_in_chars = s.graphemes(true).count();
                if length_in_chars > budget {
                    return Ok(Value::String(String::new()));
                }
            }
        }
    }

    Ok(parsed_value)
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OutputTemplate {
    Json,
    Pseudo,
    Js,
}

pub fn format_value(value: &Value, template: OutputTemplate) -> Result<String> {
    match template {
        OutputTemplate::Json => Ok(serde_json::to_string(value)?),
        OutputTemplate::Pseudo => {
            if matches!(value, Value::String(s) if s.is_empty()) {
                Ok("[ … ]".to_string())
            } else {
                Ok(serde_json::to_string(value)?)
            }
        }
        OutputTemplate::Js => {
            if matches!(value, Value::String(s) if s.is_empty()) {
                Ok("[ /* 1 more item */ ]".to_string())
            } else {
                Ok(serde_json::to_string(value)?)
            }
        }
    }
}
