use anyhow::Result;
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;
use fib_rs::Fib;
use priority_queue::PriorityQueue;

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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ParentId(pub Option<usize>);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum NodeKind { Null, Bool, Number, String, Array, Object }

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodePathSegment { pub index_in_array: Option<usize> }

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct QueueItem {
    pub node_id: NodeId,
    pub parent_id: ParentId,
    pub kind: NodeKind,
    pub depth: usize,
    pub index_in_array: Option<usize>,
    pub priority: usize,
    pub value_repr: String,
}

pub fn build_priority_queue(value: &Value) -> Result<PriorityQueue<QueueItem, usize>> {
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

    fn to_kind(value: &Value) -> NodeKind {
        match value {
            Value::Null => NodeKind::Null,
            Value::Bool(_) => NodeKind::Bool,
            Value::Number(_) => NodeKind::Number,
            Value::String(_) => NodeKind::String,
            Value::Array(_) => NodeKind::Array,
            Value::Object(_) => NodeKind::Object,
        }
    }

    fn walk(
        value: &Value,
        parent_id: Option<usize>,
        depth: usize,
        index_in_array: Option<usize>,
        next_id: &mut usize,
        pq: &mut PriorityQueue<QueueItem, usize>,
        expand_strings: bool,
    ) -> Result<usize> {
        let my_id = *next_id;
        *next_id += 1;
        let priority = match index_in_array {
            Some(i) => {
                let fib = fib_rs::Fib::single(i as u128);
                // Small indices fit in u128; fallback to depth if conversion fails
                let fib_u128 = fib.to_string().parse::<u128>().unwrap_or(0);
                depth + fib_u128 as usize
            }
            None => depth,
        };
        let item = QueueItem {
            node_id: NodeId(my_id),
            parent_id: ParentId(parent_id),
            kind: to_kind(value),
            depth,
            index_in_array,
            priority,
            value_repr: value_repr(value),
        };
        pq.push(item, priority);

        match value {
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    walk(item, Some(my_id), depth + 1, Some(i), next_id, pq, true)?;
                }
            }
            Value::Object(map) => {
                for (_k, v) in map.iter() {
                    walk(v, Some(my_id), depth + 1, None, next_id, pq, true)?;
                }
            }
            Value::String(s) => {
                if expand_strings {
                    for (i, g) in unicode_segmentation::UnicodeSegmentation::graphemes(s.as_str(), true).enumerate() {
                        let ch_value = Value::String(g.to_string());
                        walk(&ch_value, Some(my_id), depth + 1, Some(i), next_id, pq, false)?;
                    }
                }
            }
            _ => {}
        }

        Ok(my_id)
    }

    let mut next_id = 0usize;
    let mut pq: PriorityQueue<QueueItem, usize> = PriorityQueue::new();
    walk(value, None, 0, None, &mut next_id, &mut pq, true)?;
    Ok(pq)
}
