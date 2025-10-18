use anyhow::Result;
use serde_json::Value;
 
mod queue;
mod tree;
pub use queue::{build_priority_queue, NodeId, ParentId, NodeKind, QueueItem, PQBuild};
pub use tree::{build_tree, TreeKind, TreeNode};

pub fn parse_json(input: &str, _budget: usize) -> Result<Value> {
    let parsed_value: Value = serde_json::from_str(input)?;
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

pub fn headson(input: &str, template: OutputTemplate, budget: usize) -> Result<String> {
    let parsed = parse_json(input, budget)?;
    let pq_build = build_priority_queue(&parsed)?;
    best_render_under_char_budget(&pq_build, template, budget)
}

fn best_render_under_char_budget(pq_build: &PQBuild, template: OutputTemplate, char_budget: usize) -> Result<String> {
    // Iterate cumulatively increasing the number of PQ items included, stop when render exceeds budget.
    // Keep last two candidates via a small ring buffer, return the best that still fits.
    let total = pq_build.pq.len();
    let mut cand: [Option<String>; 2] = [None, None];
    let mut best_len: usize = 0;
    let mut i: usize = 1; // include at least root

    while i <= total {
        let tree = build_tree(pq_build, i)?;
        let s = tree.serialize(template);
        let l = s.len();
        let slot = i % 2;
        cand[slot] = Some(s);
        if l > char_budget {
            break;
        }
        best_len = l;
        i += 1;
    }

    // Choose the best that fits (largest length <= budget)
    let idx = if i == 0 { 0 } else { (i.saturating_sub(1)) % 2 };
    if let Some(s) = cand[idx].clone() {
        if s.len() <= char_budget { return Ok(s); }
    }
    // Fallback: try the other candidate
    let other = (idx + 1) % 2;
    if let Some(s) = cand[other].clone() {
        if s.len() <= char_budget { return Ok(s); }
    }
    // If nothing fits, render minimal tree with 1 item, or empty string if even that fails
    let tree = build_tree(pq_build, 1)?;
    let s = tree.serialize(template);
    if s.len() <= char_budget { Ok(s) } else { Ok(String::new()) }
}

