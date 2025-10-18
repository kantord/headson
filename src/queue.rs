use anyhow::Result;
use priority_queue::PriorityQueue;
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ParentId(pub Option<usize>);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum NodeKind {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct QueueItem {
    pub node_id: NodeId,
    pub parent_id: ParentId,
    pub kind: NodeKind,
    pub depth: usize,
    pub index_in_array: Option<usize>,
    pub key_in_object: Option<String>,
    pub priority: usize,
    pub value_repr: String,
}

#[derive(Clone, Debug, Default)]
pub struct NodeMetrics {
    pub array_len: Option<usize>,
    pub object_len: Option<usize>,
    pub string_len: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct PQBuild {
    pub pq: PriorityQueue<QueueItem, usize>,
    pub metrics: std::collections::HashMap<usize, NodeMetrics>,
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
    key_in_object: Option<String>,
    next_id: &mut usize,
    pq: &mut PriorityQueue<QueueItem, usize>,
    metrics: &mut std::collections::HashMap<usize, NodeMetrics>,
    expand_strings: bool,
    is_string_child: bool,
) -> Result<usize> {
    let my_id = *next_id;
    *next_id += 1;
    let priority = match index_in_array {
        Some(i) => {
            if is_string_child {
                // string-specific penalty: (max(i - 20, 1))^2
                let adj = if i > 20 { i - 20 } else { 1 };
                let penalty = adj * adj;
                depth + penalty
            } else {
                // array-specific penalty: i^3
                let penalty = i.pow(3);
                depth + penalty
            }
        }
        None => depth,
    };
    let item = QueueItem {
        node_id: NodeId(my_id),
        parent_id: ParentId(parent_id),
        kind: to_kind(value),
        depth,
        index_in_array,
        key_in_object,
        priority,
        value_repr: value_repr(value),
    };
    pq.push(item, priority);

    // Record metrics for this node
    let entry = metrics.entry(my_id).or_default();

    match value {
        Value::Array(items) => {
            entry.array_len = Some(items.len());
            for (i, item) in items.iter().enumerate() {
                walk(item, Some(my_id), depth + 1, Some(i), None, next_id, pq, metrics, true, false)?;
            }
        }
        Value::Object(map) => {
            entry.object_len = Some(map.len());
            for (k, v) in map.iter() {
                walk(v, Some(my_id), depth + 1, None, Some(k.clone()), next_id, pq, metrics, true, false)?;
            }
        }
        Value::String(s) => {
            entry.string_len = Some(UnicodeSegmentation::graphemes(s.as_str(), true).count());
            if expand_strings {
                for (i, g) in UnicodeSegmentation::graphemes(s.as_str(), true).enumerate() {
                    let ch_value = Value::String(g.to_string());
                    walk(&ch_value, Some(my_id), depth + 1, Some(i), None, next_id, pq, metrics, false, true)?;
                }
            }
        }
        _ => {}
    }

    Ok(my_id)
}

pub fn build_priority_queue(value: &Value) -> Result<PQBuild> {
    let mut next_id = 0usize;
    let mut pq: PriorityQueue<QueueItem, usize> = PriorityQueue::new();
    let mut metrics: std::collections::HashMap<usize, NodeMetrics> = std::collections::HashMap::new();
    walk(value, None, 0, None, None, &mut next_id, &mut pq, &mut metrics, true, false)?;
    Ok(PQBuild { pq, metrics })
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn pq_empty_array() {
        let value: Value = serde_json::from_str("[]").unwrap();
        let build = build_priority_queue(&value).unwrap();
        let mut lines = vec![format!("len={}", build.pq.len())];
        for (item, prio) in build.pq.into_sorted_iter() {
            lines.push(format!("{:?} prio={}", item, prio));
        }
        assert_snapshot!("pq_empty_array_queue", lines.join("\n"));
    }

    #[test]
    fn pq_single_string_array() {
        let value: Value = serde_json::from_str("[\"ab\"]").unwrap();
        let build = build_priority_queue(&value).unwrap();
        let mut lines = vec![format!("len={}", build.pq.len())];
        for (item, prio) in build.pq.into_sorted_iter() {
            lines.push(format!("{:?} prio={}", item, prio));
        }
        assert_snapshot!("pq_single_string_array_queue", lines.join("\n"));
    }
}
