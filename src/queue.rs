use anyhow::Result;
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;
use std::time::Instant;

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
    pub number_value: Option<serde_json::Number>,
    pub bool_value: Option<bool>,
    pub string_value: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct NodeMetrics {
    pub array_len: Option<usize>,
    pub object_len: Option<usize>,
    pub string_len: Option<usize>,
}

#[derive(Default, Clone, Debug)]
pub struct BuildProfile {
    pub total_nodes: usize,
    pub arrays: usize,
    pub objects: usize,
    pub strings: usize,
    pub string_chars: usize,
    pub string_enum_ns: u128,
    pub walk_ms: u128,
    pub sort_ms: u128,
    pub maps_ms: u128,
}

#[derive(Clone, Debug)]
pub struct PQBuild {
    pub metrics: Vec<NodeMetrics>,
    pub id_to_item: Vec<QueueItem>,
    pub parent_of: Vec<Option<usize>>, // parent_of[id] = parent id
    pub children_of: Vec<Vec<usize>>,  // children_of[id] = children ids
    pub order_index: Vec<usize>,       // order_index[id] = global order
    pub total_nodes: usize,
    pub profile: BuildProfile,
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

fn cumulative_walk(
    value: &Value,
    parent_id: Option<usize>,
    depth: usize,
    index_in_array: Option<usize>,
    key_in_object: Option<String>,
    next_id: &mut usize,
    out_items: &mut Vec<QueueItem>,
    metrics: &mut Vec<NodeMetrics>,
    expand_strings: bool,
    is_string_child: bool,
    parent_score: u128,
    stats: &mut BuildProfile,
) -> Result<usize> {
    let my_id = *next_id;
    *next_id += 1;
    // Cumulative scoring: score = parent_score + 1 + node_penalty
    // node_penalty:
    // - string char at index i: i + max(0, i-20)^2
    // - array item at index i: (i^3) * M (M large to dominate)
    // - others: 0
    const M: u128 = 1_000_000_000_000; // 1e12
    let node_penalty: u128 = match (index_in_array, is_string_child) {
        (Some(i), true) => {
            let extra = if i > 20 { let d = (i - 20) as u128; d * d } else { 0 };
            (i as u128) + extra
        }
        (Some(i), false) => (i as u128).pow(3) * M,
        _ => 0,
    };
    let score_u128 = parent_score + 1 + node_penalty;
    let priority: usize = if score_u128 > usize::MAX as u128 { usize::MAX } else { score_u128 as usize };
    let item = QueueItem {
        node_id: NodeId(my_id),
        parent_id: ParentId(parent_id),
        kind: to_kind(value),
        depth,
        index_in_array,
        key_in_object,
        priority,
        value_repr: value_repr(value),
        number_value: match value { Value::Number(n) => Some(n.clone()), _ => None },
        bool_value: match value { Value::Bool(b) => Some(*b), _ => None },
        string_value: match value { Value::String(s) => Some(s.clone()), _ => None },
    };
    out_items.push(item);
    stats.total_nodes += 1;

    // Record metrics for this node. Avoid holding a mutable borrow across
    // recursive calls to satisfy the borrow checker.
    // Helper to ensure metrics has an entry for this id
    if metrics.len() <= my_id { metrics.resize(my_id + 1, NodeMetrics::default()); }

    match value {
        Value::Array(items) => {
            metrics[my_id].array_len = Some(items.len());
            stats.arrays += 1;
            for (i, item) in items.iter().enumerate() {
                cumulative_walk(item, Some(my_id), depth + 1, Some(i), None, next_id, out_items, metrics, true, false, score_u128, stats)?;
            }
        }
        Value::Object(map) => {
            metrics[my_id].object_len = Some(map.len());
            stats.objects += 1;
            for (k, v) in map.iter() {
                cumulative_walk(v, Some(my_id), depth + 1, None, Some(k.clone()), next_id, out_items, metrics, true, false, score_u128, stats)?;
            }
        }
        Value::String(s) => {
            stats.strings += 1;
            if expand_strings {
                let t_chars = Instant::now();
                let mut len = 0usize;
                for (i, g) in UnicodeSegmentation::graphemes(s.as_str(), true).enumerate() {
                    len = i + 1;
                    // Inline fast-path for string character nodes to avoid constructing a serde_json::Value
                    let child_id = *next_id;
                    *next_id += 1;
                    // Penalty for string chars: i + max(0, i-20)^2
                    let extra = if i > 20 { let d = (i - 20) as u128; d * d } else { 0 };
                    let char_penalty: u128 = (i as u128) + extra;
                    let child_score = score_u128 + 1 + char_penalty;
                    let priority: usize = if child_score > usize::MAX as u128 { usize::MAX } else { child_score as usize };
                    let item = QueueItem {
                        node_id: NodeId(child_id),
                        parent_id: ParentId(Some(my_id)),
                        kind: NodeKind::String,
                        depth: depth + 1,
                        index_in_array: Some(i),
                        key_in_object: None,
                        priority,
                        value_repr: format!("\"{}\"", g),
                        number_value: None,
                        bool_value: None,
                        string_value: Some(g.to_string()),
                    };
                    out_items.push(item);
                    if metrics.len() <= child_id { metrics.resize(child_id + 1, NodeMetrics::default()); }
                    metrics[child_id].string_len = Some(1);
                    stats.total_nodes += 1;
                    stats.string_chars += 1;
                }
                metrics[my_id].string_len = Some(len);
                stats.string_enum_ns += t_chars.elapsed().as_nanos();
            } else {
                let count = UnicodeSegmentation::graphemes(s.as_str(), true).count();
                metrics[my_id].string_len = Some(count);
            }
        }
        _ => {}
    }

    Ok(my_id)
}

pub fn build_priority_queue(value: &Value) -> Result<PQBuild> {
    let mut next_id = 0usize;
    let mut flat_items: Vec<QueueItem> = Vec::new();
    let mut metrics: Vec<NodeMetrics> = Vec::new();
    let mut stats = BuildProfile::default();
    let t_walk = std::time::Instant::now();
    cumulative_walk(value, None, 0, None, None, &mut next_id, &mut flat_items, &mut metrics, true, false, 0u128, &mut stats)?;
    stats.walk_ms = t_walk.elapsed().as_millis() as u128;
    // Build arena-like Vecs
    let total = next_id;
    let mut id_to_item_opt: Vec<Option<QueueItem>> = vec![None; total];
    let mut parent_of: Vec<Option<usize>> = vec![None; total];
    let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); total];
    let mut order_index: Vec<usize> = vec![usize::MAX; total];

    // Stable order index by ascending priority
    let t_sort = std::time::Instant::now();
    flat_items.sort_by_key(|it| it.priority);
    stats.sort_ms = t_sort.elapsed().as_millis() as u128;
    let t_maps = std::time::Instant::now();
    for (idx, it) in flat_items.iter().cloned().enumerate() {
        let id = it.node_id.0;
        order_index[id] = idx;
        parent_of[id] = it.parent_id.0;
        id_to_item_opt[id] = Some(it.clone());
        if let Some(pid) = it.parent_id.0 {
            children_of[pid].push(id);
        }
    }
    // Convert id_to_item to a dense Vec
    let id_to_item: Vec<QueueItem> = id_to_item_opt.into_iter().map(|o| o.expect("missing queue item by id")).collect();
    // Ensure children are ordered by index_in_array when relevant
    for kids in children_of.iter_mut() {
        kids.sort_by_key(|cid| id_to_item[*cid].index_in_array.unwrap_or(usize::MAX));
    }
    stats.maps_ms = t_maps.elapsed().as_millis() as u128;

    Ok(PQBuild { metrics, id_to_item, parent_of, children_of, order_index, total_nodes: next_id, profile: stats })
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn pq_empty_array() {
        let value: Value = serde_json::from_str("[]").unwrap();
        let build = build_priority_queue(&value).unwrap();
        let mut items_sorted: Vec<_> = build.id_to_item.iter().cloned().collect();
        items_sorted.sort_by_key(|it| build.order_index.get(it.node_id.0).copied().unwrap_or(usize::MAX));
        let mut lines = vec![format!("len={}", build.total_nodes)];
        for it in items_sorted { lines.push(format!("{:?} prio={}", it, it.priority)); }
        assert_snapshot!("pq_empty_array_queue", lines.join("\n"));
    }

    #[test]
    fn pq_single_string_array() {
        let value: Value = serde_json::from_str("[\"ab\"]").unwrap();
        let build = build_priority_queue(&value).unwrap();
        let mut items_sorted: Vec<_> = build.id_to_item.iter().cloned().collect();
        items_sorted.sort_by_key(|it| build.order_index.get(it.node_id.0).copied().unwrap_or(usize::MAX));
        let mut lines = vec![format!("len={}", build.total_nodes)];
        for it in items_sorted { lines.push(format!("{:?} prio={}", it, it.priority)); }
        assert_snapshot!("pq_single_string_array_queue", lines.join("\n"));
    }
}
