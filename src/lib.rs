use anyhow::Result;
use serde_json::Value;
 
mod queue;
mod tree;
mod render;
pub use queue::{build_priority_queue, NodeId, ParentId, NodeKind, QueueItem, PQBuild};
pub use tree::{build_tree, TreeKind, TreeNode};

#[cfg(test)]
#[path = "."]
mod generated_tests {
    include!(env!("JSON_SUITE_GEN"));
}

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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct RenderConfig {
    pub template: OutputTemplate,
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

pub fn headson(input: &str, config: RenderConfig, budget: usize) -> Result<String> {
    let parsed = parse_json(input, budget)?;
    let pq_build = build_priority_queue(&parsed)?;
    best_render_under_char_budget(&pq_build, config, budget)
}

fn best_render_under_char_budget(pq_build: &PQBuild, config: RenderConfig, char_budget: usize) -> Result<String> {
    // Binary search the largest k in [1, total] whose render fits into char_budget
    let total = pq_build.pq.len();
    if total == 0 { return Ok(String::new()); }
    let mut lo = 1usize;
    let mut hi = total;
    let mut best: Option<String> = None;

    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let tree = build_tree(pq_build, mid)?;
        let s = tree.serialize(&config);
        if s.len() <= char_budget {
            best = Some(s);
            lo = mid + 1;
        } else {
            hi = mid.saturating_sub(1);
        }
    }

    Ok(best.unwrap_or_default())
}

