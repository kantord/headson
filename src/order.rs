use anyhow::Result;
use unicode_segmentation::UnicodeSegmentation;

use std::cmp::Reverse;
use std::collections::BinaryHeap;

#[derive(Clone, Debug)]
pub struct PriorityConfig {
    pub max_string_graphemes: usize,
    pub array_max_items: usize,
}

impl PriorityConfig {
    pub fn new(max_string_graphemes: usize, array_max_items: usize) -> Self {
        Self {
            max_string_graphemes,
            array_max_items,
        }
    }
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
pub struct RankedNode {
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

#[derive(Clone, Debug)]
pub struct PriorityOrder {
    pub metrics: Vec<NodeMetrics>,
    pub id_to_item: Vec<RankedNode>,
    pub parent_of: Vec<Option<usize>>, // parent_of[id] = parent id
    pub children_of: Vec<Vec<usize>>,  // children_of[id] = children ids
    pub order_index: Vec<usize>,       // order_index[id] = global order
    pub ids_by_order: Vec<usize>,      // ids sorted by ascending priority
    pub total_nodes: usize,
}

pub const ROOT_PQ_ID: usize = 0;

// No aliases; canonical names only

// Frontier builder from streaming arena (Stage 2 adapter)
use crate::stream_arena::StreamArena;

#[derive(Clone)]
struct Entry {
    score: u128,
    pq_id: usize,
    kind: NodeKind,
    depth: usize,
    arena_node: Option<usize>,
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.pq_id == other.pq_id
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score
            .cmp(&other.score)
            .then_with(|| self.pq_id.cmp(&other.pq_id))
    }
}

/// Hard ceiling on number of PQ nodes built to prevent degenerate inputs
/// from blowing up memory/time while exploring the frontier.
const SAFETY_CAP: usize = 2_000_000;

// Priority scoring knobs
//
// We build a single global priority order by walking the parsed arena with a
// min-heap over a monotonic "score". Lower scores come first in the order.
// These constants shape how we interleave arrays/objects/strings during the walk.

/// Root starts at a fixed minimal score so its children naturally follow.
const ROOT_BASE_SCORE: u128 = 1;

/// Small base increment so array children follow the parent.
const ARRAY_CHILD_BASE_INCREMENT: u128 = 1;
/// Strong cubic index term to bias earlier array items far ahead of later ones.
/// The large multiplier ensures array index dominates depth ties.
const ARRAY_INDEX_CUBIC_WEIGHT: u128 = 1_000_000_000_000;

/// Small base increment so object properties appear right after their object.
const OBJECT_CHILD_BASE_INCREMENT: u128 = 1;

/// Base increment so string grapheme expansions follow their parent string.
const STRING_CHILD_BASE_INCREMENT: u128 = 1;
/// Linear weight to prefer earlier graphemes strongly.
const STRING_CHILD_LINEAR_WEIGHT: u128 = 1;
/// Index after which we penalize graphemes quadratically to de-prioritize
/// very deep string expansions vs. structural nodes.
const STRING_INDEX_INFLECTION: usize = 20;
/// Quadratic penalty multiplier for string grapheme expansions beyond the
/// inflection point.
const STRING_INDEX_QUADRATIC_WEIGHT: u128 = 1;

struct Scope<'a> {
    arena: &'a StreamArena,
    config: &'a PriorityConfig,
    next_pq_id: &'a mut usize,
    parent_of: &'a mut Vec<Option<usize>>,
    children_of: &'a mut Vec<Vec<usize>>,
    metrics: &'a mut Vec<NodeMetrics>,
    id_to_item: &'a mut Vec<RankedNode>,
    heap: &'a mut BinaryHeap<Reverse<Entry>>,
    safety_cap: usize,
}

impl<'a> Scope<'a> {
    fn record_array_metrics(&mut self, id: usize, arena_id: usize) {
        let array_len = self.arena.nodes[arena_id]
            .array_len
            .unwrap_or(self.arena.nodes[arena_id].children_len);
        self.metrics[id].array_len = Some(array_len);
    }

    fn record_object_metrics(&mut self, id: usize, arena_id: usize) {
        let object_len = self.arena.nodes[arena_id]
            .object_len
            .unwrap_or(self.arena.nodes[arena_id].children_len);
        self.metrics[id].object_len = Some(object_len);
    }

    fn record_string_metrics(&mut self, id: usize) {
        let s = self.id_to_item[id].string_value.as_deref().unwrap_or("");
        let mut iter = UnicodeSegmentation::graphemes(s, true);
        let count =
            iter.by_ref().take(self.config.max_string_graphemes).count();
        self.metrics[id].string_len = Some(count);
        if iter.next().is_some() {
            self.metrics[id].string_truncated = true;
        }
    }

    fn record_metrics_for(
        &mut self,
        id: usize,
        kind: &NodeKind,
        arena_id: usize,
    ) {
        match kind {
            NodeKind::Array => self.record_array_metrics(id, arena_id),
            NodeKind::Object => self.record_object_metrics(id, arena_id),
            NodeKind::String => self.record_string_metrics(id),
            _ => {}
        }
    }

    fn expand_array_children(&mut self, entry: &Entry, arena_id: usize) {
        let id = entry.pq_id;
        let node = &self.arena.nodes[arena_id];
        let kept = node.children_len;
        for i in 0..kept {
            let child_arena_id = self.arena.children[node.children_start + i];
            let child_kind = self.arena.nodes[child_arena_id].kind.clone();
            let child_pq = *self.next_pq_id;
            *self.next_pq_id += 1;
            self.parent_of.push(Some(id));
            self.children_of.push(Vec::new());
            self.metrics.push(NodeMetrics::default());
            let extra = (i as u128).pow(3) * ARRAY_INDEX_CUBIC_WEIGHT;
            let score = entry.score + ARRAY_CHILD_BASE_INCREMENT + extra;
            let child_node = &self.arena.nodes[child_arena_id];
            self.id_to_item.push(RankedNode {
                node_id: NodeId(child_pq),
                parent_id: ParentId(Some(id)),
                kind: child_kind.clone(),
                depth: entry.depth + 1,
                index_in_array: Some(i),
                key_in_object: None,
                priority: clamp_score(score),
                number_value: child_node.number_value.clone(),
                bool_value: child_node.bool_value,
                string_value: child_node.string_value.clone(),
            });
            self.children_of[id].push(child_pq);
            self.heap.push(Reverse(Entry {
                score,
                pq_id: child_pq,
                kind: child_kind,
                depth: entry.depth + 1,
                arena_node: Some(child_arena_id),
            }));
            if *self.next_pq_id >= self.safety_cap {
                break;
            }
        }
    }

    fn expand_object_children(&mut self, entry: &Entry, arena_id: usize) {
        let id = entry.pq_id;
        let node = &self.arena.nodes[arena_id];
        let mut items: Vec<(String, usize, usize)> =
            Vec::with_capacity(node.children_len);
        for i in 0..node.children_len {
            let key = self.arena.obj_keys[node.obj_keys_start + i].clone();
            let child_arena_id = self.arena.children[node.children_start + i];
            items.push((key, child_arena_id, i));
        }
        items.sort_by(|a, b| a.0.cmp(&b.0));
        for (key, child_arena_id, _i) in items {
            let child_kind = self.arena.nodes[child_arena_id].kind.clone();
            let child_pq = *self.next_pq_id;
            *self.next_pq_id += 1;
            self.parent_of.push(Some(id));
            self.children_of.push(Vec::new());
            self.metrics.push(NodeMetrics::default());
            let score = entry.score + OBJECT_CHILD_BASE_INCREMENT;
            let child_node = &self.arena.nodes[child_arena_id];
            self.id_to_item.push(RankedNode {
                node_id: NodeId(child_pq),
                parent_id: ParentId(Some(id)),
                kind: child_kind.clone(),
                depth: entry.depth + 1,
                index_in_array: None,
                key_in_object: Some(key.clone()),
                priority: clamp_score(score),
                number_value: child_node.number_value.clone(),
                bool_value: child_node.bool_value,
                string_value: child_node.string_value.clone(),
            });
            self.children_of[id].push(child_pq);
            self.heap.push(Reverse(Entry {
                score,
                pq_id: child_pq,
                kind: child_kind,
                depth: entry.depth + 1,
                arena_node: Some(child_arena_id),
            }));
            if *self.next_pq_id >= self.safety_cap {
                break;
            }
        }
    }

    fn expand_string_children(&mut self, entry: &Entry) {
        let id = entry.pq_id;
        let full =
            self.id_to_item[id].string_value.clone().unwrap_or_default();
        let mut iter = UnicodeSegmentation::graphemes(full.as_str(), true);
        for (i, _g) in iter
            .by_ref()
            .take(self.config.max_string_graphemes)
            .enumerate()
        {
            let child_pq = *self.next_pq_id;
            *self.next_pq_id += 1;
            self.parent_of.push(Some(id));
            self.children_of.push(Vec::new());
            self.metrics.push(NodeMetrics::default());
            let extra = if i > STRING_INDEX_INFLECTION {
                let d = (i - STRING_INDEX_INFLECTION) as u128;
                d * d * STRING_INDEX_QUADRATIC_WEIGHT
            } else {
                0
            };
            let score = entry.score
                + STRING_CHILD_BASE_INCREMENT
                + (i as u128) * STRING_CHILD_LINEAR_WEIGHT
                + extra;
            self.id_to_item.push(RankedNode {
                node_id: NodeId(child_pq),
                parent_id: ParentId(Some(id)),
                kind: NodeKind::String,
                depth: entry.depth + 1,
                index_in_array: Some(i),
                key_in_object: None,
                priority: clamp_score(score),
                number_value: None,
                bool_value: None,
                string_value: None,
            });
            self.children_of[id].push(child_pq);
            self.heap.push(Reverse(Entry {
                score,
                pq_id: child_pq,
                kind: NodeKind::String,
                depth: entry.depth + 1,
                arena_node: None,
            }));
            if *self.next_pq_id >= self.safety_cap {
                break;
            }
        }
    }

    fn process_entry(&mut self, entry: &Entry, ids_by_order: &mut Vec<usize>) {
        let id = entry.pq_id;
        ids_by_order.push(id);
        if let Some(ar_id) = entry.arena_node {
            self.record_metrics_for(id, &entry.kind, ar_id);
        }
        match entry.kind {
            NodeKind::Array => {
                if let Some(ar_id) = entry.arena_node {
                    self.expand_array_children(entry, ar_id);
                }
            }
            NodeKind::Object => {
                if let Some(ar_id) = entry.arena_node {
                    self.expand_object_children(entry, ar_id);
                }
            }
            NodeKind::String => {
                self.expand_string_children(entry);
            }
            _ => {}
        }
    }
}

fn clamp_score(score: u128) -> usize {
    if score > usize::MAX as u128 {
        usize::MAX
    } else {
        score as usize
    }
}

pub fn build_priority_order_from_arena(
    arena: &StreamArena,
    config: &PriorityConfig,
) -> Result<PriorityOrder> {
    let t_walk = std::time::Instant::now();
    let mut next_pq_id: usize = 0;
    let mut id_to_item: Vec<RankedNode> = Vec::new();
    let mut parent_of: Vec<Option<usize>> = Vec::new();
    let mut children_of: Vec<Vec<usize>> = Vec::new();
    let mut metrics: Vec<NodeMetrics> = Vec::new();
    let mut ids_by_order: Vec<usize> = Vec::new();
    let mut heap: BinaryHeap<Reverse<Entry>> = BinaryHeap::new();

    // Seed root from arena
    let root_ar = arena.root_id;
    let root_kind = arena.nodes[root_ar].kind.clone();
    let root_pq = next_pq_id;
    next_pq_id += 1;
    parent_of.push(None);
    children_of.push(Vec::new());
    metrics.push(NodeMetrics::default());
    let n = &arena.nodes[root_ar];
    id_to_item.push(RankedNode {
        node_id: NodeId(root_pq),
        parent_id: ParentId(None),
        kind: root_kind.clone(),
        depth: 0,
        index_in_array: None,
        key_in_object: None,
        priority: clamp_score(ROOT_BASE_SCORE),
        number_value: n.number_value.clone(),
        bool_value: n.bool_value,
        string_value: n.string_value.clone(),
    });
    heap.push(Reverse(Entry {
        score: ROOT_BASE_SCORE,
        pq_id: root_pq,
        kind: root_kind,
        depth: 0,
        arena_node: Some(root_ar),
    }));
    // root counted implicitly via next_pq_id

    while let Some(Reverse(entry)) = heap.pop() {
        let mut scope = Scope {
            arena,
            config,
            next_pq_id: &mut next_pq_id,
            parent_of: &mut parent_of,
            children_of: &mut children_of,
            metrics: &mut metrics,
            id_to_item: &mut id_to_item,
            heap: &mut heap,
            safety_cap: SAFETY_CAP,
        };
        scope.process_entry(&entry, &mut ids_by_order);
        if next_pq_id >= SAFETY_CAP {
            break;
        }
    }

    let _walk_ms = t_walk.elapsed().as_millis();
    let total = next_pq_id;
    let mut order_index: Vec<usize> = vec![usize::MAX; total];
    for (idx, &pid) in ids_by_order.iter().enumerate() {
        if pid < total {
            order_index[pid] = idx;
        }
    }

    Ok(PriorityOrder {
        metrics,
        id_to_item,
        parent_of,
        children_of,
        order_index,
        ids_by_order,
        total_nodes: total,
    })
}

// No alias; use `build_priority_order_from_arena`

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn order_empty_array() {
        let arena = crate::stream_arena::build_stream_arena(
            "[]",
            &PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_priority_order_from_arena(
            &arena,
            &PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut items_sorted: Vec<_> = build.id_to_item.clone();
        items_sorted.sort_by_key(|it| {
            build
                .order_index
                .get(it.node_id.0)
                .copied()
                .unwrap_or(usize::MAX)
        });
        let mut lines = vec![format!("len={}", build.total_nodes)];
        for it in items_sorted {
            lines.push(format!("{:?} prio={}", it, it.priority));
        }
        assert_snapshot!("order_empty_array_order", lines.join("\n"));
    }

    #[test]
    fn order_single_string_array() {
        let arena = crate::stream_arena::build_stream_arena(
            "[\"ab\"]",
            &PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_priority_order_from_arena(
            &arena,
            &PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut items_sorted: Vec<_> = build.id_to_item.clone();
        items_sorted.sort_by_key(|it| {
            build
                .order_index
                .get(it.node_id.0)
                .copied()
                .unwrap_or(usize::MAX)
        });
        let mut lines = vec![format!("len={}", build.total_nodes)];
        for it in items_sorted {
            lines.push(format!("{:?} prio={}", it, it.priority));
        }
        assert_snapshot!("order_single_string_array_order", lines.join("\n"));
    }
}
