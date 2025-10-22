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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId(pub usize);

impl From<usize> for NodeId {
    fn from(value: usize) -> Self {
        NodeId(value)
    }
}

impl From<NodeId> for usize {
    fn from(value: NodeId) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ParentId(pub Option<NodeId>);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
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
    pub key_in_object: Option<String>,
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
    // All ids in this structure are PQ ids (0..total_nodes).
    // They correspond to `NodeId.0` in `RankedNode` for convenience when indexing.
    pub parent_of: Vec<Option<NodeId>>, // parent_of[id] = parent id (PQ id)
    pub children_of: Vec<Vec<NodeId>>, // children_of[id] = children ids (PQ ids)
    pub ids_by_order: Vec<NodeId>, // ids sorted by ascending priority (PQ ids)
    pub total_nodes: usize,
}

pub const ROOT_PQ_ID: usize = 0;

use crate::utils::tree_arena::JsonTreeArena;

#[derive(Clone)]
struct Entry {
    score: u128,
    pq_id: usize,
    depth: usize,
    // When present, we can read kind from the arena node.
    // When None, this is a synthetic entry (currently only string grapheme).
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
    arena: &'a JsonTreeArena,
    config: &'a PriorityConfig,
    next_pq_id: &'a mut usize,
    parent_of: &'a mut Vec<Option<NodeId>>,
    children_of: &'a mut Vec<Vec<NodeId>>,
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
        kind: NodeKind,
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
            let child_kind = self.arena.nodes[child_arena_id].kind;
            let child_pq = *self.next_pq_id;
            *self.next_pq_id += 1;
            self.parent_of.push(Some(NodeId(id)));
            self.children_of.push(Vec::new());
            self.metrics.push(NodeMetrics::default());
            let extra = (i as u128).pow(3) * ARRAY_INDEX_CUBIC_WEIGHT;
            let score = entry.score + ARRAY_CHILD_BASE_INCREMENT + extra;
            let child_node = &self.arena.nodes[child_arena_id];
            self.id_to_item.push(RankedNode {
                node_id: NodeId(child_pq),
                parent_id: ParentId(Some(NodeId(id))),
                kind: child_kind,
                depth: entry.depth + 1,
                key_in_object: None,
                number_value: child_node.number_value.clone(),
                bool_value: child_node.bool_value,
                string_value: child_node.string_value.clone(),
            });
            self.children_of[id].push(NodeId(child_pq));
            self.heap.push(Reverse(Entry {
                score,
                pq_id: child_pq,
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
        // Collect pairs of (key_index_in_arena, child_arena_id) without cloning keys
        let mut items: Vec<(usize, usize)> =
            Vec::with_capacity(node.children_len);
        for i in 0..node.children_len {
            let key_idx = node.obj_keys_start + i;
            let child_arena_id = self.arena.children[node.children_start + i];
            items.push((key_idx, child_arena_id));
        }
        // Sort by key string lexicographically using borrowed &str
        items.sort_by(|a, b| {
            let ka = &self.arena.obj_keys[a.0];
            let kb = &self.arena.obj_keys[b.0];
            ka.cmp(kb)
        });
        for (key_idx, child_arena_id) in items {
            let child_kind = self.arena.nodes[child_arena_id].kind;
            let child_pq = *self.next_pq_id;
            *self.next_pq_id += 1;
            self.parent_of.push(Some(NodeId(id)));
            self.children_of.push(Vec::new());
            self.metrics.push(NodeMetrics::default());
            let score = entry.score + OBJECT_CHILD_BASE_INCREMENT;
            let child_node = &self.arena.nodes[child_arena_id];
            self.id_to_item.push(RankedNode {
                node_id: NodeId(child_pq),
                parent_id: ParentId(Some(NodeId(id))),
                kind: child_kind,
                depth: entry.depth + 1,
                key_in_object: Some(self.arena.obj_keys[key_idx].clone()),
                number_value: child_node.number_value.clone(),
                bool_value: child_node.bool_value,
                string_value: child_node.string_value.clone(),
            });
            self.children_of[id].push(NodeId(child_pq));
            self.heap.push(Reverse(Entry {
                score,
                pq_id: child_pq,
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
        let full = self.id_to_item[id].string_value.as_deref().unwrap_or("");
        let count = UnicodeSegmentation::graphemes(full, true)
            .take(self.config.max_string_graphemes)
            .count();
        for i in 0..count {
            let child_pq = *self.next_pq_id;
            *self.next_pq_id += 1;
            self.parent_of.push(Some(NodeId(id)));
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
                parent_id: ParentId(Some(NodeId(id))),
                kind: NodeKind::String,
                depth: entry.depth + 1,
                key_in_object: None,
                number_value: None,
                bool_value: None,
                string_value: None,
            });
            self.children_of[id].push(NodeId(child_pq));
            self.heap.push(Reverse(Entry {
                score,
                pq_id: child_pq,
                depth: entry.depth + 1,
                arena_node: None,
            }));
            if *self.next_pq_id >= self.safety_cap {
                break;
            }
        }
    }

    fn resolve_kind(&self, entry: &Entry) -> NodeKind {
        if let Some(ar_id) = entry.arena_node {
            self.arena.nodes[ar_id].kind
        } else {
            NodeKind::String
        }
    }

    fn expand_for(&mut self, entry: &Entry, kind: NodeKind) {
        match kind {
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
            NodeKind::String => self.expand_string_children(entry),
            _ => {}
        }
    }

    fn process_entry(
        &mut self,
        entry: &Entry,
        ids_by_order: &mut Vec<NodeId>,
    ) {
        let id = entry.pq_id;
        ids_by_order.push(NodeId(id));
        let kind = self.resolve_kind(entry);
        if let Some(ar_id) = entry.arena_node {
            self.record_metrics_for(id, kind, ar_id);
        }
        self.expand_for(entry, kind);
    }
}

pub fn build_priority_order_from_arena(
    arena: &JsonTreeArena,
    config: &PriorityConfig,
) -> Result<PriorityOrder> {
    let t_walk = std::time::Instant::now();
    let mut next_pq_id: usize = 0;
    let mut id_to_item: Vec<RankedNode> = Vec::new();
    let mut parent_of: Vec<Option<NodeId>> = Vec::new();
    let mut children_of: Vec<Vec<NodeId>> = Vec::new();
    let mut metrics: Vec<NodeMetrics> = Vec::new();
    let mut ids_by_order: Vec<NodeId> = Vec::new();
    let mut heap: BinaryHeap<Reverse<Entry>> = BinaryHeap::new();

    // Seed root from arena
    let root_ar = arena.root_id;
    let root_kind = arena.nodes[root_ar].kind;
    let root_pq = next_pq_id;
    next_pq_id += 1;
    parent_of.push(None);
    children_of.push(Vec::new());
    metrics.push(NodeMetrics::default());
    let n = &arena.nodes[root_ar];
    id_to_item.push(RankedNode {
        node_id: NodeId(root_pq),
        parent_id: ParentId(None),
        kind: root_kind,
        depth: 0,
        key_in_object: None,
        number_value: n.number_value.clone(),
        bool_value: n.bool_value,
        string_value: n.string_value.clone(),
    });
    heap.push(Reverse(Entry {
        score: ROOT_BASE_SCORE,
        pq_id: root_pq,
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
    Ok(PriorityOrder {
        metrics,
        id_to_item,
        parent_of,
        children_of,
        ids_by_order,
        total_nodes: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn order_empty_array() {
        let arena = crate::json_ingest::build_json_tree_arena(
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
        // Build a transient mapping from id -> order index
        let mut order_index = vec![usize::MAX; build.total_nodes];
        for (idx, &pid) in build.ids_by_order.iter().enumerate() {
            let pidx = pid.0;
            if pidx < build.total_nodes {
                order_index[pidx] = idx;
            }
        }
        items_sorted.sort_by_key(|it| {
            order_index.get(it.node_id.0).copied().unwrap_or(usize::MAX)
        });
        let mut lines = vec![format!("len={}", build.total_nodes)];
        for it in items_sorted {
            lines.push(format!("{it:?}"));
        }
        assert_snapshot!("order_empty_array_order", lines.join("\n"));
    }

    #[test]
    fn order_single_string_array() {
        let arena = crate::json_ingest::build_json_tree_arena(
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
        let mut order_index = vec![usize::MAX; build.total_nodes];
        for (idx, &pid) in build.ids_by_order.iter().enumerate() {
            let pidx = pid.0;
            if pidx < build.total_nodes {
                order_index[pidx] = idx;
            }
        }
        items_sorted.sort_by_key(|it| {
            order_index.get(it.node_id.0).copied().unwrap_or(usize::MAX)
        });
        let mut lines = vec![format!("len={}", build.total_nodes)];
        for it in items_sorted {
            lines.push(format!("{it:?}"));
        }
        assert_snapshot!("order_single_string_array_order", lines.join("\n"));
    }
}
