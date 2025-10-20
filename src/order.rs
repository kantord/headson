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

#[derive(Default, Clone, Debug)]
pub struct BuildProfile {
    pub total_nodes: usize,
    pub arrays: usize,
    pub objects: usize,
    pub strings: usize,
    pub string_chars: usize,
    pub walk_ms: u128,
    // Extra diagnostics
    pub arrays_items_total: usize,
    pub objects_props_total: usize,
    pub max_array_len: usize,
    pub max_object_len: usize,
    pub max_string_len: usize,
    pub children_edges_total: usize,
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
    pub profile: BuildProfile,
}

// No aliases; canonical names only

// Frontier builder from streaming arena (Stage 2 adapter)
use crate::stream_arena::StreamArena;

pub fn build_priority_order_from_arena(
    arena: &StreamArena,
    cfg: &PriorityConfig,
) -> Result<PriorityOrder> {
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

    let mut stats = BuildProfile::default();
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
        priority: 1usize,
        number_value: n.number_value.clone(),
        bool_value: n.bool_value,
        string_value: n.string_value.clone(),
    });
    heap.push(Reverse(Entry {
        score: 1,
        pq_id: root_pq,
        kind: root_kind,
        depth: 0,
        arena_node: Some(root_ar),
    }));
    stats.total_nodes += 1;

    // Safety cap to prevent runaway expansion on adversarial inputs.
    // Large enough to exceed any realistic budget while keeping memory bounded.
    let safety_cap: usize = 2_000_000;

    // Helpers to keep complexity low inside the main loop.
    #[allow(clippy::too_many_arguments)]
    fn record_metrics_for(
        kind: &NodeKind,
        ar_id: usize,
        arena: &StreamArena,
        id: usize,
        id_to_item: &[RankedNode],
        metrics: &mut [NodeMetrics],
        stats: &mut BuildProfile,
        cfg: &PriorityConfig,
    ) {
        match kind {
            NodeKind::Array => {
                let alen = arena.nodes[ar_id]
                    .array_len
                    .unwrap_or(arena.nodes[ar_id].children_len);
                metrics[id].array_len = Some(alen);
                stats.arrays += 1;
                stats.arrays_items_total += alen;
                if alen > stats.max_array_len {
                    stats.max_array_len = alen;
                }
            }
            NodeKind::Object => {
                let olen = arena.nodes[ar_id]
                    .object_len
                    .unwrap_or(arena.nodes[ar_id].children_len);
                metrics[id].object_len = Some(olen);
                stats.objects += 1;
                stats.objects_props_total += olen;
                if olen > stats.max_object_len {
                    stats.max_object_len = olen;
                }
            }
            NodeKind::String => {
                stats.strings += 1;
                let s = id_to_item[id].string_value.as_deref().unwrap_or("");
                let mut iter = UnicodeSegmentation::graphemes(s, true);
                let count =
                    iter.by_ref().take(cfg.max_string_graphemes).count();
                metrics[id].string_len = Some(count);
                if iter.next().is_some() {
                    metrics[id].string_truncated = true;
                }
                stats.string_chars += count;
                if count > stats.max_string_len {
                    stats.max_string_len = count;
                }
            }
            _ => {}
        }
    }

    fn clamp_score(score: u128) -> usize {
        if score > usize::MAX as u128 {
            usize::MAX
        } else {
            score as usize
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn expand_array_children(
        entry: &Entry,
        ar_id: usize,
        arena: &StreamArena,
        next_pq_id: &mut usize,
        parent_of: &mut Vec<Option<usize>>,
        children_of: &mut Vec<Vec<usize>>,
        metrics: &mut Vec<NodeMetrics>,
        id_to_item: &mut Vec<RankedNode>,
        heap: &mut BinaryHeap<Reverse<Entry>>,
        stats: &mut BuildProfile,
        safety_cap: usize,
    ) {
        let id = entry.pq_id;
        let node = &arena.nodes[ar_id];
        let kept = node.children_len;
        for i in 0..kept {
            let child_ar = arena.children[node.children_start + i];
            let child_kind = arena.nodes[child_ar].kind.clone();
            let child_pq = *next_pq_id;
            *next_pq_id += 1;
            parent_of.push(Some(id));
            children_of.push(Vec::new());
            metrics.push(NodeMetrics::default());
            let extra = (i as u128).pow(3) * 1_000_000_000_000u128;
            let score = entry.score + 1 + extra;
            let cn = &arena.nodes[child_ar];
            id_to_item.push(RankedNode {
                node_id: NodeId(child_pq),
                parent_id: ParentId(Some(id)),
                kind: child_kind.clone(),
                depth: entry.depth + 1,
                index_in_array: Some(i),
                key_in_object: None,
                priority: clamp_score(score),
                number_value: cn.number_value.clone(),
                bool_value: cn.bool_value,
                string_value: cn.string_value.clone(),
            });
            children_of[id].push(child_pq);
            heap.push(Reverse(Entry {
                score,
                pq_id: child_pq,
                kind: child_kind,
                depth: entry.depth + 1,
                arena_node: Some(child_ar),
            }));
            stats.total_nodes += 1;
            if *next_pq_id >= safety_cap {
                break;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn expand_object_children(
        entry: &Entry,
        ar_id: usize,
        arena: &StreamArena,
        next_pq_id: &mut usize,
        parent_of: &mut Vec<Option<usize>>,
        children_of: &mut Vec<Vec<usize>>,
        metrics: &mut Vec<NodeMetrics>,
        id_to_item: &mut Vec<RankedNode>,
        heap: &mut BinaryHeap<Reverse<Entry>>,
        stats: &mut BuildProfile,
        safety_cap: usize,
    ) {
        let id = entry.pq_id;
        let node = &arena.nodes[ar_id];
        let mut items: Vec<(String, usize, usize)> =
            Vec::with_capacity(node.children_len);
        for i in 0..node.children_len {
            let key = arena.obj_keys[node.obj_keys_start + i].clone();
            let child_ar = arena.children[node.children_start + i];
            items.push((key, child_ar, i));
        }
        items.sort_by(|a, b| a.0.cmp(&b.0));
        for (key, child_ar, _i) in items {
            let child_kind = arena.nodes[child_ar].kind.clone();
            let child_pq = *next_pq_id;
            *next_pq_id += 1;
            parent_of.push(Some(id));
            children_of.push(Vec::new());
            metrics.push(NodeMetrics::default());
            let score = entry.score + 1;
            let cn = &arena.nodes[child_ar];
            id_to_item.push(RankedNode {
                node_id: NodeId(child_pq),
                parent_id: ParentId(Some(id)),
                kind: child_kind.clone(),
                depth: entry.depth + 1,
                index_in_array: None,
                key_in_object: Some(key.clone()),
                priority: clamp_score(score),
                number_value: cn.number_value.clone(),
                bool_value: cn.bool_value,
                string_value: cn.string_value.clone(),
            });
            children_of[id].push(child_pq);
            heap.push(Reverse(Entry {
                score,
                pq_id: child_pq,
                kind: child_kind,
                depth: entry.depth + 1,
                arena_node: Some(child_ar),
            }));
            stats.total_nodes += 1;
            if *next_pq_id >= safety_cap {
                break;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn expand_string_children(
        entry: &Entry,
        id_to_item: &mut Vec<RankedNode>,
        cfg: &PriorityConfig,
        next_pq_id: &mut usize,
        parent_of: &mut Vec<Option<usize>>,
        children_of: &mut Vec<Vec<usize>>,
        metrics: &mut Vec<NodeMetrics>,
        heap: &mut BinaryHeap<Reverse<Entry>>,
        stats: &mut BuildProfile,
        safety_cap: usize,
    ) {
        let id = entry.pq_id;
        let s = id_to_item[id].string_value.clone().unwrap_or_default();
        let mut iter = UnicodeSegmentation::graphemes(s.as_str(), true);
        for (i, _g) in iter.by_ref().take(cfg.max_string_graphemes).enumerate()
        {
            let child_pq = *next_pq_id;
            *next_pq_id += 1;
            parent_of.push(Some(id));
            children_of.push(Vec::new());
            metrics.push(NodeMetrics::default());
            let extra = if i > 20 {
                let d = (i - 20) as u128;
                d * d
            } else {
                0
            };
            let score = entry.score + 1 + (i as u128) + extra;
            id_to_item.push(RankedNode {
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
            children_of[id].push(child_pq);
            heap.push(Reverse(Entry {
                score,
                pq_id: child_pq,
                kind: NodeKind::String,
                depth: entry.depth + 1,
                arena_node: None,
            }));
            stats.total_nodes += 1;
            if *next_pq_id >= safety_cap {
                break;
            }
        }
    }

    while let Some(Reverse(entry)) = heap.pop() {
        let id = entry.pq_id;
        ids_by_order.push(id);

        if let Some(ar_id) = entry.arena_node {
            record_metrics_for(
                &entry.kind,
                ar_id,
                arena,
                id,
                &id_to_item,
                &mut metrics,
                &mut stats,
                cfg,
            );
        }

        match entry.kind.clone() {
            NodeKind::Array => {
                if let Some(ar_id) = entry.arena_node {
                    expand_array_children(
                        &entry,
                        ar_id,
                        arena,
                        &mut next_pq_id,
                        &mut parent_of,
                        &mut children_of,
                        &mut metrics,
                        &mut id_to_item,
                        &mut heap,
                        &mut stats,
                        safety_cap,
                    );
                }
            }
            NodeKind::Object => {
                if let Some(ar_id) = entry.arena_node {
                    expand_object_children(
                        &entry,
                        ar_id,
                        arena,
                        &mut next_pq_id,
                        &mut parent_of,
                        &mut children_of,
                        &mut metrics,
                        &mut id_to_item,
                        &mut heap,
                        &mut stats,
                        safety_cap,
                    );
                }
            }
            NodeKind::String => {
                expand_string_children(
                    &entry,
                    &mut id_to_item,
                    cfg,
                    &mut next_pq_id,
                    &mut parent_of,
                    &mut children_of,
                    &mut metrics,
                    &mut heap,
                    &mut stats,
                    safety_cap,
                );
            }
            _ => {}
        }
        if next_pq_id >= safety_cap {
            break;
        }
    }

    stats.walk_ms = t_walk.elapsed().as_millis();
    let total = next_pq_id;
    let mut order_index: Vec<usize> = vec![usize::MAX; total];
    for (idx, &pid) in ids_by_order.iter().enumerate() {
        if pid < total {
            order_index[pid] = idx;
        }
    }
    stats.children_edges_total = children_of.iter().map(Vec::len).sum();

    Ok(PriorityOrder {
        metrics,
        id_to_item,
        parent_of,
        children_of,
        order_index,
        ids_by_order,
        total_nodes: total,
        profile: stats,
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
