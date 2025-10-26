use anyhow::Result;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use unicode_segmentation::UnicodeSegmentation;

use super::scoring::*;
use super::types::*;
use crate::utils::tree_arena::JsonTreeArena;

#[derive(Clone)]
struct Entry {
    score: u128,
    // Index into the priority-ordered nodes (0..total_nodes)
    priority_index: usize,
    depth: usize,
    // When present, we can read kind from the arena (parsed JSON) node.
    // When None, this is a synthetic entry (currently only string grapheme).
    arena_index: Option<usize>,
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
            && self.priority_index == other.priority_index
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
            .then_with(|| self.priority_index.cmp(&other.priority_index))
    }
}

struct CommonChild {
    arena_index: Option<usize>,
    score: u128,
    ranked: RankedNode,
    index_in_parent_array: Option<usize>,
}

struct Scope<'a> {
    arena: &'a JsonTreeArena,
    config: &'a PriorityConfig,
    next_pq_id: &'a mut usize,
    parent: &'a mut Vec<Option<NodeId>>,
    children: &'a mut Vec<Vec<NodeId>>,
    metrics: &'a mut Vec<NodeMetrics>,
    nodes: &'a mut Vec<RankedNode>,
    heap: &'a mut BinaryHeap<Reverse<Entry>>,
    safety_cap: usize,
    object_type: &'a mut Vec<ObjectType>,
    index_in_parent_array: &'a mut Vec<Option<usize>>,
}

impl<'a> Scope<'a> {
    fn push_child_common(
        &mut self,
        entry: &Entry,
        child_priority_index: usize,
        common: CommonChild,
    ) {
        let id = entry.priority_index;
        self.parent.push(Some(NodeId(id)));
        self.children.push(Vec::new());
        self.metrics.push(NodeMetrics::default());
        self.nodes.push(common.ranked);
        self.index_in_parent_array
            .push(common.index_in_parent_array);
        // Children created from parsing regular JSON are standard objects/arrays/etc.
        // If child is an object, default to Object type.
        self.object_type.push(ObjectType::Object);
        self.children[id].push(NodeId(child_priority_index));
        self.heap.push(Reverse(Entry {
            score: common.score,
            priority_index: child_priority_index,
            depth: entry.depth + 1,
            arena_index: common.arena_index,
        }));
    }
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
        let s = self.nodes[id].string_value.as_deref().unwrap_or("");
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

    #[allow(
        clippy::cognitive_complexity,
        reason = "Array child scoring includes simple bias selection branches"
    )]
    fn expand_array_children(&mut self, entry: &Entry, arena_id: usize) {
        let node = &self.arena.nodes[arena_id];
        let kept = node.children_len;
        for i in 0..kept {
            let child_arena_id = self.arena.children[node.children_start + i];
            let child_kind = self.arena.nodes[child_arena_id].kind;
            let child_priority_index = *self.next_pq_id;
            *self.next_pq_id += 1;
            // Original index in source array if tracked; fall back to kept index.
            let orig_index = if node.arr_indices_len > 0 {
                let start = node.arr_indices_start;
                self.arena.arr_indices[start + i]
            } else {
                i
            };
            let extra: u128 = if self.config.prefer_tail_arrays {
                let idx_for_priority: usize =
                    kept.saturating_sub(1).saturating_sub(i);
                let ii = idx_for_priority as u128;
                ii * ii * ii * ARRAY_INDEX_CUBIC_WEIGHT
            } else {
                match self.config.array_bias {
                    super::types::ArrayBias::Head => {
                        let ii = i as u128;
                        ii * ii * ii * ARRAY_INDEX_CUBIC_WEIGHT
                    }
                    super::types::ArrayBias::HeadMidTail => {
                        let mid = kept.saturating_sub(1) / 2;
                        let d_head = i as isize;
                        let d_tail =
                            kept.saturating_sub(1) as isize - i as isize;
                        let d_mid = (i as isize - mid as isize).abs();
                        let d = d_head.min(d_tail).min(d_mid).unsigned_abs()
                            as u128;
                        d * d * d * ARRAY_INDEX_CUBIC_WEIGHT
                    }
                }
            };
            let score = entry.score + ARRAY_CHILD_BASE_INCREMENT + extra;
            let child_node = &self.arena.nodes[child_arena_id];
            self.push_child_common(
                entry,
                child_priority_index,
                CommonChild {
                    arena_index: Some(child_arena_id),
                    score,
                    ranked: RankedNode {
                        node_id: NodeId(child_priority_index),
                        kind: child_kind,
                        key_in_object: None,
                        number_value: child_node.number_value.clone(),
                        bool_value: child_node.bool_value,
                        string_value: child_node.string_value.clone(),
                    },
                    index_in_parent_array: Some(orig_index),
                },
            );
            if *self.next_pq_id >= self.safety_cap {
                break;
            }
        }
    }

    fn expand_object_children(&mut self, entry: &Entry, arena_id: usize) {
        let node = &self.arena.nodes[arena_id];
        let mut items: Vec<(usize, usize)> =
            Vec::with_capacity(node.children_len);
        for i in 0..node.children_len {
            let key_idx = node.obj_keys_start + i;
            let child_arena_id = self.arena.children[node.children_start + i];
            items.push((key_idx, child_arena_id));
        }
        items.sort_by(|a, b| {
            self.arena.obj_keys[a.0].cmp(&self.arena.obj_keys[b.0])
        });
        for (key_idx, child_arena_id) in items {
            let child_kind = self.arena.nodes[child_arena_id].kind;
            let child_priority_index = *self.next_pq_id;
            *self.next_pq_id += 1;
            let score = entry.score + OBJECT_CHILD_BASE_INCREMENT;
            let child_node = &self.arena.nodes[child_arena_id];
            self.push_child_common(
                entry,
                child_priority_index,
                CommonChild {
                    arena_index: Some(child_arena_id),
                    score,
                    ranked: RankedNode {
                        node_id: NodeId(child_priority_index),
                        kind: child_kind,
                        key_in_object: Some(
                            self.arena.obj_keys[key_idx].clone(),
                        ),
                        number_value: child_node.number_value.clone(),
                        bool_value: child_node.bool_value,
                        string_value: child_node.string_value.clone(),
                    },
                    index_in_parent_array: None,
                },
            );
            if *self.next_pq_id >= self.safety_cap {
                break;
            }
        }
    }

    fn expand_string_children(&mut self, entry: &Entry) {
        let id = entry.priority_index;
        let full = self.nodes[id].string_value.as_deref().unwrap_or("");
        let count = UnicodeSegmentation::graphemes(full, true)
            .take(self.config.max_string_graphemes)
            .count();
        for i in 0..count {
            let child_priority_index = *self.next_pq_id;
            *self.next_pq_id += 1;
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
            self.push_child_common(
                entry,
                child_priority_index,
                CommonChild {
                    arena_index: None,
                    score,
                    ranked: RankedNode {
                        node_id: NodeId(child_priority_index),
                        kind: NodeKind::String,
                        key_in_object: None,
                        number_value: None,
                        bool_value: None,
                        string_value: None,
                    },
                    index_in_parent_array: None,
                },
            );
            if *self.next_pq_id >= self.safety_cap {
                break;
            }
        }
    }

    fn resolve_kind(&self, entry: &Entry) -> NodeKind {
        if let Some(ar_id) = entry.arena_index {
            self.arena.nodes[ar_id].kind
        } else {
            NodeKind::String
        }
    }

    fn expand_for(&mut self, entry: &Entry, kind: NodeKind) {
        match kind {
            NodeKind::Array => {
                if let Some(ar_id) = entry.arena_index {
                    self.expand_array_children(entry, ar_id);
                }
            }
            NodeKind::Object => {
                if let Some(ar_id) = entry.arena_index {
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
        let id = entry.priority_index;
        ids_by_order.push(NodeId(id));
        let kind = self.resolve_kind(entry);
        if let Some(ar_id) = entry.arena_index {
            self.record_metrics_for(id, kind, ar_id);
        }
        self.expand_for(entry, kind);
    }
}

pub fn build_order(
    arena: &JsonTreeArena,
    config: &PriorityConfig,
) -> Result<PriorityOrder> {
    let mut next_pq_id: usize = 0;
    let mut nodes: Vec<RankedNode> = Vec::new();
    let mut parent: Vec<Option<NodeId>> = Vec::new();
    let mut children: Vec<Vec<NodeId>> = Vec::new();
    let mut metrics: Vec<NodeMetrics> = Vec::new();
    let mut order: Vec<NodeId> = Vec::new();
    let mut object_type: Vec<ObjectType> = Vec::new();
    let mut heap: BinaryHeap<Reverse<Entry>> = BinaryHeap::new();
    let mut index_in_parent_array: Vec<Option<usize>> = Vec::new();

    // Seed root from arena
    let root_ar = arena.root_id;
    let root_kind = arena.nodes[root_ar].kind;
    let root_priority_index = next_pq_id;
    next_pq_id += 1;
    parent.push(None);
    children.push(Vec::new());
    metrics.push(NodeMetrics::default());
    index_in_parent_array.push(None);
    let n = &arena.nodes[root_ar];
    nodes.push(RankedNode {
        node_id: NodeId(root_priority_index),
        kind: root_kind,
        key_in_object: None,
        number_value: n.number_value.clone(),
        bool_value: n.bool_value,
        string_value: n.string_value.clone(),
    });
    // Root object type: mark fileset root specially, otherwise Object.
    let root_ot = if arena.is_fileset {
        ObjectType::Fileset
    } else {
        ObjectType::Object
    };
    object_type.push(root_ot);
    heap.push(Reverse(Entry {
        score: ROOT_BASE_SCORE,
        priority_index: root_priority_index,
        depth: 0,
        arena_index: Some(root_ar),
    }));

    while let Some(Reverse(entry)) = heap.pop() {
        let mut scope = Scope {
            arena,
            config,
            next_pq_id: &mut next_pq_id,
            parent: &mut parent,
            children: &mut children,
            metrics: &mut metrics,
            nodes: &mut nodes,
            heap: &mut heap,
            safety_cap: SAFETY_CAP,
            object_type: &mut object_type,
            index_in_parent_array: &mut index_in_parent_array,
        };
        scope.process_entry(&entry, &mut order);
        if next_pq_id >= SAFETY_CAP {
            break;
        }
    }

    let total = next_pq_id;
    Ok(PriorityOrder {
        metrics,
        nodes,
        parent,
        children,
        index_in_parent_array,
        by_priority: order,
        total_nodes: total,
        object_type,
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
        let build = super::build_order(
            &arena,
            &PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut items_sorted: Vec<_> = build.nodes.clone();
        // Build a transient mapping from id -> by_priority index
        let mut order_index = vec![usize::MAX; build.total_nodes];
        for (idx, &pid) in build.by_priority.iter().enumerate() {
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
        let build = super::build_order(
            &arena,
            &PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut items_sorted: Vec<_> = build.nodes.clone();
        let mut order_index = vec![usize::MAX; build.total_nodes];
        for (idx, &pid) in build.by_priority.iter().enumerate() {
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
