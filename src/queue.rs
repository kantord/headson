use anyhow::Result;
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;
// use std::time::Instant; // removed unused; timings gathered elsewhere
const MAX_STRING_ENUM: usize = 500;

use std::cmp::Reverse;
use std::collections::BinaryHeap;

#[derive(Clone, Debug)]
pub struct PQConfig {
    pub max_string_graphemes: usize,
}

impl Default for PQConfig {
    fn default() -> Self { Self { max_string_graphemes: MAX_STRING_ENUM } }
}

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
    pub number_value: Option<serde_json::Number>,
    pub bool_value: Option<bool>,
    pub string_value: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct NodeMetrics {
    pub array_len: Option<usize>,
    pub object_len: Option<usize>,
    pub string_len: Option<usize>,
    pub string_truncated: bool,
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
    // Extra diagnostics
    pub arrays_items_total: usize,
    pub objects_props_total: usize,
    pub max_array_len: usize,
    pub max_object_len: usize,
    pub max_string_len: usize,
    pub long_strings_over_1k: usize,
    pub long_strings_over_10k: usize,
    pub children_edges_total: usize,
    pub map_fill_ns: u128,
    pub child_sort_ns: u128,
}

#[derive(Clone, Debug)]
pub struct PQBuild {
    pub metrics: Vec<NodeMetrics>,
    pub id_to_item: Vec<QueueItem>,
    pub parent_of: Vec<Option<usize>>, // parent_of[id] = parent id
    pub children_of: Vec<Vec<usize>>,  // children_of[id] = children ids
    pub order_index: Vec<usize>,       // order_index[id] = global order
    pub ids_by_order: Vec<usize>,      // ids sorted by ascending priority
    pub total_nodes: usize,
    pub profile: BuildProfile,
}

// value_repr removed; we now keep only typed values as needed.

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

pub fn build_priority_queue(value: &Value) -> Result<PQBuild> {
    let default_cfg = PQConfig::default();
    build_priority_queue_with_config(value, &default_cfg)
}

pub fn build_priority_queue_with_config(value: &Value, cfg: &PQConfig) -> Result<PQBuild> {
    // Frontier-based top-K build
    #[derive(Clone)]
    #[allow(dead_code)]
    enum VRef<'a> { Json(&'a Value), StrChar }

    #[derive(Clone)]
    #[allow(dead_code)]
    struct Entry<'a> {
        score: u128,
        id: usize,
        parent: Option<usize>,
        vref: VRef<'a>,
        kind: NodeKind,
        depth: usize,
        index_in_array: Option<usize>,
        key_in_object: Option<String>,
    }
    impl<'a> PartialEq for Entry<'a> { fn eq(&self, other: &Self) -> bool { self.score == other.score && self.id == other.id } }
    impl<'a> Eq for Entry<'a> {}
    impl<'a> PartialOrd for Entry<'a> { fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) } }
    impl<'a> Ord for Entry<'a> {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            // Reverse for min-heap via BinaryHeap of Reverse
            self.score.cmp(&other.score).then_with(|| self.id.cmp(&other.id))
        }
    }

    let mut stats = BuildProfile::default();
    let t_walk = std::time::Instant::now();
    let mut next_id: usize = 0;
    let mut id_to_item: Vec<QueueItem> = Vec::new();
    let mut parent_of: Vec<Option<usize>> = Vec::new();
    let mut children_of: Vec<Vec<usize>> = Vec::new();
    let mut metrics: Vec<NodeMetrics> = Vec::new();
    let mut ids_by_order: Vec<usize> = Vec::new();
    let mut heap: BinaryHeap<Reverse<Entry>> = BinaryHeap::new();

    // Seed root
    let root_id = next_id; next_id += 1;
    parent_of.push(None);
    children_of.push(Vec::new());
    metrics.push(NodeMetrics::default());
    id_to_item.push(QueueItem{
        node_id: NodeId(root_id), parent_id: ParentId(None), kind: to_kind(value), depth: 0,
        index_in_array: None, key_in_object: None, priority: 1usize, number_value: match value { Value::Number(n)=>Some(n.clone()), _=>None },
        bool_value: match value { Value::Bool(b)=>Some(*b), _=>None }, string_value: match value { Value::String(s)=>Some(s.clone()), _=>None }
    });
    heap.push(Reverse(Entry{ score: 1, id: root_id, parent: None, vref: VRef::Json(value), kind: to_kind(value), depth: 0, index_in_array: None, key_in_object: None }));
    stats.total_nodes += 1;

    // We will build up to K nodes, where K ~ ids upper bound we'll need (use a big hint; caller uses char budget)
    // For compatibility with current binary search we can generate up to a conservative upper bound: 2*char_budget
    // But we don't have budget here; we assume caller will pass a PQConfig and rely on probes over ids_by_order length.
    // Heuristically generate up to 2_000_000 nodes or until heap empty; but better: generate until heap empty or a safety cap.
    let safety_cap: usize = 2_000_000; // conservative

    while let Some(Reverse(entry)) = heap.pop() {
        let id = entry.id;
        ids_by_order.push(id);
        // Record metrics for parent node quickly
        let kind_now1 = entry.kind.clone();
        match (&entry.vref, kind_now1) {
            (VRef::Json(Value::Array(items)), _) => { metrics[id].array_len = Some(items.len()); stats.arrays += 1; stats.arrays_items_total += items.len(); if items.len() > stats.max_array_len { stats.max_array_len = items.len(); } }
            (VRef::Json(Value::Object(map)), _) => { metrics[id].object_len = Some(map.len()); stats.objects += 1; stats.objects_props_total += map.len(); if map.len() > stats.max_object_len { stats.max_object_len = map.len(); } }
            (VRef::Json(Value::String(s)), _) => { stats.strings += 1; let mut iter = UnicodeSegmentation::graphemes(s.as_str(), true); let count = iter.by_ref().take(cfg.max_string_graphemes).count(); metrics[id].string_len = Some(count); if iter.next().is_some() { metrics[id].string_truncated = true; } stats.string_chars += count; if count > stats.max_string_len { stats.max_string_len = count; } }
            _ => {}
        }

        // Expand children lazily
        let kind_now2 = entry.kind.clone();
        match (&entry.vref, kind_now2) {
            (VRef::Json(Value::Array(items)), NodeKind::Array) => {
                for (i, child) in items.iter().enumerate() {
                    let child_id = next_id; next_id += 1;
                    parent_of.push(Some(id)); children_of.push(Vec::new()); metrics.push(NodeMetrics::default());
                    let score = entry.score + 1 + (i as u128).pow(3) * 1_000_000_000_000u128;
                    id_to_item.push(QueueItem{
                        node_id: NodeId(child_id), parent_id: ParentId(Some(id)), kind: to_kind(child), depth: entry.depth+1,
                        index_in_array: Some(i), key_in_object: None, priority: if score>usize::MAX as u128 {usize::MAX} else {score as usize},
                        number_value: match child { Value::Number(n)=>Some(n.clone()), _=>None },
                        bool_value: match child { Value::Bool(b)=>Some(*b), _=>None },
                        string_value: match child { Value::String(s)=>Some(s.clone()), _=>None }
                    });
                    children_of[id].push(child_id);
                    heap.push(Reverse(Entry{ score, id: child_id, parent: Some(id), vref: VRef::Json(child), kind: to_kind(child), depth: entry.depth+1, index_in_array: Some(i), key_in_object: None }));
                    stats.total_nodes += 1;
                    if next_id >= safety_cap { break; }
                }
            }
            (VRef::Json(Value::Object(map)), NodeKind::Object) => {
                for (k, v) in map.iter() {
                    let child_id = next_id; next_id += 1;
                    parent_of.push(Some(id)); children_of.push(Vec::new()); metrics.push(NodeMetrics::default());
                    let score = entry.score + 1; // no penalty
                    id_to_item.push(QueueItem{
                        node_id: NodeId(child_id), parent_id: ParentId(Some(id)), kind: to_kind(v), depth: entry.depth+1,
                        index_in_array: None, key_in_object: Some(k.clone()), priority: if score>usize::MAX as u128 {usize::MAX} else {score as usize},
                        number_value: match v { Value::Number(n)=>Some(n.clone()), _=>None },
                        bool_value: match v { Value::Bool(b)=>Some(*b), _=>None },
                        string_value: match v { Value::String(s)=>Some(s.clone()), _=>None }
                    });
                    children_of[id].push(child_id);
                    heap.push(Reverse(Entry{ score, id: child_id, parent: Some(id), vref: VRef::Json(v), kind: to_kind(v), depth: entry.depth+1, index_in_array: None, key_in_object: Some(k.clone()) }));
                    stats.total_nodes += 1;
                    if next_id >= safety_cap { break; }
                }
            }
            (VRef::Json(Value::String(s)), NodeKind::String) => {
                // Expand grapheme children up to cfg.max_string_graphemes
                let mut iter = UnicodeSegmentation::graphemes(s.as_str(), true);
                for (i, _g) in iter.by_ref().take(cfg.max_string_graphemes).enumerate() {
                    let child_id = next_id; next_id += 1;
                    parent_of.push(Some(id)); children_of.push(Vec::new()); metrics.push(NodeMetrics::default());
                    let extra = if i > 20 { let d = (i - 20) as u128; d*d } else { 0 };
                    let score = entry.score + 1 + (i as u128) + extra;
                    id_to_item.push(QueueItem{
                        node_id: NodeId(child_id), parent_id: ParentId(Some(id)), kind: NodeKind::String, depth: entry.depth+1,
                        index_in_array: Some(i), key_in_object: None, priority: if score>usize::MAX as u128 {usize::MAX} else {score as usize},
                        number_value: None, bool_value: None, string_value: None
                    });
                    children_of[id].push(child_id);
                    // No need to carry actual Value for char; use StrChar
                    heap.push(Reverse(Entry{ score, id: child_id, parent: Some(id), vref: VRef::StrChar, kind: NodeKind::String, depth: entry.depth+1, index_in_array: Some(i), key_in_object: None }));
                    stats.total_nodes += 1;
                    if next_id >= safety_cap { break; }
                }
                stats.string_enum_ns += 0; // accounted above via metrics.string_len and string_chars
            }
            _ => {}
        }
        if next_id >= safety_cap { break; }
    }
    stats.walk_ms = t_walk.elapsed().as_millis() as u128;

    // Build order_index directly from ids_by_order
    let total = next_id;
    let mut order_index: Vec<usize> = vec![usize::MAX; total];
    for (idx, &id) in ids_by_order.iter().enumerate() { if id < total { order_index[id] = idx; } }
    // Edge count and maps timing trivial here
    stats.children_edges_total = children_of.iter().map(|v| v.len()).sum();
    // Compute ids_by_order complete; no separate sort time now
    stats.sort_ms = 0;
    stats.maps_ms = 0;

    Ok(PQBuild { metrics, id_to_item, parent_of, children_of, order_index, ids_by_order, total_nodes: total, profile: stats })
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
