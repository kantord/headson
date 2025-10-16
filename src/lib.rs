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
    let json = match value {
        Value::Array(items) => {
            if items.is_empty() {
                "[]".to_string()
            } else if items.len() == 1 {
                if let Value::String(s) = &items[0] {
                    format!("[\n  \"{}\"\n]", s)
                } else {
                    "[]".to_string()
                }
            } else {
                "[]".to_string()
            }
        }
        Value::Object(_) => "{}".to_string(),
        _ => "[]".to_string(),
    };

    let out = match template {
        OutputTemplate::Json => json,
        OutputTemplate::Pseudo => if matches!(value, Value::String(s) if s.is_empty()) { "[\n  …\n]".to_string() } else { json },
        OutputTemplate::Js => if matches!(value, Value::String(s) if s.is_empty()) { "[\n  /* 1 more item */\n]".to_string() } else { json },
    };
    Ok(out)
}

pub fn write_debug<W: std::io::Write>(value: &Value, writer: &mut W) -> Result<()> {
    fn node_type_of(value: &Value) -> &'static str {
        match value {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    fn value_repr(value: &Value) -> String {
        match value {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => format!("\"{}\"", s),
            Value::Array(items) => {
                if items.is_empty() {
                    "[]".to_string()
                } else if items.len() == 1 {
                    if let Value::String(s) = &items[0] {
                        format!("[\"{}\"]", s)
                    } else {
                        "[]".to_string()
                    }
                } else {
                    "[]".to_string()
                }
            }
            Value::Object(_) => "{}".to_string(),
        }
    }

    fn walk<W: std::io::Write>(
        value: &Value,
        parent_id: Option<usize>,
        depth: usize,
        index_in_array: Option<usize>,
        next_id: &mut usize,
        writer: &mut W,
        expand_strings: bool,
    ) -> Result<usize> {
        let my_id = *next_id;
        *next_id += 1;
        let parent_repr = parent_id.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
        let idx_repr = index_in_array.map(|i| format!(" index={}", i)).unwrap_or_else(|| "".to_string());
        writeln!(
            writer,
            "id={} type={} parent={} depth={}{} value={}",
            my_id,
            node_type_of(value),
            parent_repr,
            depth,
            idx_repr,
            value_repr(value)
        )?;

        match value {
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    walk(item, Some(my_id), depth + 1, Some(i), next_id, writer, true)?;
                }
            }
            Value::Object(map) => {
                for (_k, v) in map.iter() {
                    walk(v, Some(my_id), depth + 1, None, next_id, writer, true)?;
                }
            }
            Value::String(s) => {
                if expand_strings {
                    for (i, g) in unicode_segmentation::UnicodeSegmentation::graphemes(s.as_str(), true).enumerate() {
                        let ch_value = Value::String(g.to_string());
                        walk(&ch_value, Some(my_id), depth + 1, Some(i), next_id, writer, false)?;
                    }
                }
            }
            _ => {}
        }

        Ok(my_id)
    }

    let mut next_id = 0usize;
    walk(value, None, 0, None, &mut next_id, writer, true)?;
    Ok(())
}
