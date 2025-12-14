use anyhow::Result;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

use super::scoring::*;
use super::types::*;
use crate::utils::tree_arena::{JsonTreeArena, JsonTreeNode};

fn fill_fileset_slot(
    arena: &JsonTreeArena,
    slots: &mut [Option<usize>],
    start: usize,
    slot: usize,
) {
    let mut stack = vec![start];
    while let Some(node_id) = stack.pop() {
        if slots[node_id].is_some() {
            continue;
        }
        slots[node_id] = Some(slot);
        let node = &arena.nodes[node_id];
        for idx in 0..node.children_len {
            let next = arena.children[node.children_start + idx];
            stack.push(next);
        }
    }
}

fn compute_fileset_slots(arena: &JsonTreeArena) -> Option<Vec<Option<usize>>> {
    if !arena.is_fileset {
        return None;
    }
    let root = arena.root_id;
    let root_node = &arena.nodes[root];
    let mut slots: Vec<Option<usize>> = vec![None; arena.nodes.len()];
    for slot in 0..root_node.children_len {
        let child_id = arena.children[root_node.children_start + slot];
        fill_fileset_slot(arena, &mut slots, child_id, slot);
    }
    Some(slots)
}

fn split_priority_by_slot(
    by_priority: &[NodeId],
    node_slots: &[Option<usize>],
    file_count: usize,
) -> (Vec<NodeId>, Vec<VecDeque<NodeId>>) {
    let mut buckets: Vec<VecDeque<NodeId>> = vec![VecDeque::new(); file_count];
    let mut prefix: Vec<NodeId> = Vec::new();
    for &node in by_priority {
        if let Some(slot) = node_slots.get(node.0).copied().flatten() {
            if let Some(bucket) = buckets.get_mut(slot) {
                bucket.push_back(node);
            } else {
                prefix.push(node);
            }
        } else {
            prefix.push(node);
        }
    }
    (prefix, buckets)
}

fn collect_round_robin(
    mut prefix: Vec<NodeId>,
    mut buckets: Vec<VecDeque<NodeId>>,
    capacity: usize,
) -> Vec<NodeId> {
    let mut new_order: Vec<NodeId> = Vec::with_capacity(capacity);
    new_order.append(&mut prefix);
    loop {
        let mut pushed = false;
        for bucket in buckets.iter_mut() {
            if let Some(node) = bucket.pop_front() {
                new_order.push(node);
                pushed = true;
            }
        }
        if !pushed {
            break;
        }
    }
    new_order
}

/// Reorder `by_priority` so each fileset contributes one node before any file
/// gets a second turn. This keeps tight budgets from starving later files.
#[allow(
    clippy::cognitive_complexity,
    reason = "Small bucket shuffle helper reads clearer inline than split"
)]
fn interleave_fileset_priority(
    by_priority: &mut Vec<NodeId>,
    node_slots: &[Option<usize>],
    file_count: usize,
    file_roots: &[NodeId],
) {
    if file_count == 0 {
        return;
    }
    let (prefix, buckets) =
        split_priority_by_slot(by_priority, node_slots, file_count);
    let mut buckets = buckets;
    for (slot, bucket) in buckets.iter_mut().enumerate() {
        if let Some(root) = file_roots.get(slot) {
            if let Some(pos) = bucket.iter().position(|id| id == root) {
                if let Some(root_id) = bucket.remove(pos) {
                    bucket.push_front(root_id);
                }
            } else {
                bucket.push_front(*root);
            }
        }
    }
    let new_order = collect_round_robin(prefix, buckets, by_priority.len());
    *by_priority = new_order;
}

#[derive(Default)]
struct DuplicateCounts {
    global: HashMap<String, usize>,
    per_file: Option<Vec<HashMap<String, usize>>>,
}

impl DuplicateCounts {
    fn count_for(&self, token: &str, slot: Option<usize>) -> usize {
        if let Some(per_file) = &self.per_file {
            if let Some(s) = slot.and_then(|s| per_file.get(s)) {
                return *s.get(token).unwrap_or(&0);
            }
        }
        *self.global.get(token).unwrap_or(&0)
    }
}

#[derive(Clone)]
struct Entry {
    score: u128,
    // Index into the priority-ordered nodes (0..total_nodes)
    priority_index: usize,
    depth: usize,
    // When present, we can read kind from the arena (parsed JSON) node.
    // When None, this is a synthetic entry (used for string grapheme entries).
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
    scores: &'a mut Vec<u128>,
    heap: &'a mut BinaryHeap<Reverse<Entry>>,
    safety_cap: usize,
    object_type: &'a mut Vec<ObjectType>,
    index_in_parent_array: &'a mut Vec<Option<usize>>,
    arena_to_pq: &'a mut Vec<Option<usize>>,
    node_slots: &'a mut Vec<Option<usize>>,
    arena_slots: Option<&'a [Option<usize>]>,
    duplicate_counts: &'a DuplicateCounts,
}

impl<'a> Scope<'a> {
    fn parent_is_fileset_child(&self, parent_id: usize) -> bool {
        self.parent
            .get(parent_id)
            .and_then(|p| *p)
            .and_then(|pid| self.object_type.get(pid.0))
            .is_some_and(|ot| *ot == ObjectType::Fileset)
    }

    fn zero_bias_for_code_parent(
        &self,
        parent_is_code_array: bool,
        entry: &Entry,
    ) -> bool {
        parent_is_code_array
            && self.config.line_budget_only
            && (entry.depth == 0
                || self.parent_is_fileset_child(entry.priority_index))
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "Code-scoring heuristics are easier to follow inline despite branching."
    )]
    fn adjust_code_child_score(
        &self,
        mut score: u128,
        parent_is_code_array: bool,
        child_kind: NodeKind,
        child_node: &JsonTreeNode,
        child_arena_id: usize,
    ) -> Option<u128> {
        if !parent_is_code_array {
            return Some(score);
        }
        let mut dup_count = 0usize;
        if self.config.line_budget_only
            && child_kind == NodeKind::Array
            && code_array_is_brace_only(self.arena, child_arena_id)
        {
            return None;
        }
        if let Some(token) = child_node.atomic_token.as_ref() {
            let slot = self.arena_slots.and_then(|slots| {
                slots.get(child_arena_id).copied().flatten()
            });
            dup_count = self.duplicate_counts.count_for(token.trim(), slot);
            if dup_count > 1 {
                score = score.saturating_add(CODE_DUPLICATE_LINE_PENALTY);
            }
        }
        if matches!(
            child_kind,
            NodeKind::Null | NodeKind::Bool | NodeKind::Number
        ) {
            if let Some(token) = child_node.atomic_token.as_ref() {
                if dup_count > 1 && code_line_length_extreme(token) {
                    score = score.saturating_add(CODE_EXTREME_LINE_PENALTY);
                }
                if code_line_is_brace_only(token) {
                    score = score.saturating_add(CODE_BRACE_ONLY_PENALTY);
                }
            }
        } else if child_kind == NodeKind::Array
            && self.config.line_budget_only
            && child_node.children_len <= 2
        {
            score = score.saturating_add(CODE_SHALLOW_ARRAY_PENALTY);
        }
        Some(score)
    }

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
        self.scores.push(common.score);
        self.index_in_parent_array
            .push(common.index_in_parent_array);
        // Children created from parsing regular JSON are standard objects/arrays/etc.
        // If child is an object, default to Object type.
        self.object_type.push(ObjectType::Object);
        self.children[id].push(NodeId(child_priority_index));
        if let Some(arena_idx) = common.arena_index {
            if arena_idx >= self.arena_to_pq.len() {
                self.arena_to_pq.resize(arena_idx + 1, None);
            }
            self.arena_to_pq[arena_idx] = Some(child_priority_index);
        }
        let slot = common.arena_index.and_then(|idx| {
            self.arena_slots
                .and_then(|slots| slots.get(idx).copied().flatten())
        });
        self.node_slots.push(slot);
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
        let s = match &self.nodes[id] {
            RankedNode::SplittableLeaf { value, .. } => value.as_str(),
            _ => unreachable!(
                "record_string_metrics called for non-string node: id={id}"
            ),
        };
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

    fn resolved_bias(
        &self,
        arena_id: Option<usize>,
        depth: usize,
    ) -> super::types::ArrayBias {
        let mut bias = self.config.array_bias;
        if let Some(aid) = arena_id {
            if let Some(override_bias) =
                self.arena.nodes[aid].array_bias_override
            {
                bias = override_bias;
            }
        }
        if matches!(bias, super::types::ArrayBias::HeadTail) && depth == 0 {
            super::types::ArrayBias::HeadMidTail
        } else {
            bias
        }
    }

    fn bias_extra(
        bias: super::types::ArrayBias,
        i: usize,
        kept: usize,
    ) -> u128 {
        match bias {
            super::types::ArrayBias::Head => {
                let ii = i as u128;
                ii * ii * ii * ARRAY_INDEX_CUBIC_WEIGHT
            }
            super::types::ArrayBias::HeadMidTail => {
                let mid_hi = kept.saturating_sub(1) / 2;
                let mid_lo = kept / 2;
                let d_head = i as isize;
                let d_tail = kept.saturating_sub(1) as isize - i as isize;
                let d_mid_hi = (i as isize - mid_hi as isize).abs();
                let d_mid_lo = (i as isize - mid_lo as isize).abs();
                let d_mid = d_mid_hi.min(d_mid_lo);
                let d = d_head.min(d_tail).min(d_mid).unsigned_abs() as u128;
                d * d * d * ARRAY_INDEX_CUBIC_WEIGHT
            }
            super::types::ArrayBias::HeadTail => {
                let d_head = i as isize;
                let d_tail = kept.saturating_sub(1) as isize - i as isize;
                let d = d_head.min(d_tail).unsigned_abs() as u128;
                d * d * d * ARRAY_INDEX_CUBIC_WEIGHT
            }
        }
    }

    fn array_extra_for_index(
        &self,
        i: usize,
        kept: usize,
        arena_id: Option<usize>,
        depth: usize,
    ) -> u128 {
        if self.config.prefer_tail_arrays {
            let idx_for_priority = kept.saturating_sub(1).saturating_sub(i);
            let ii = idx_for_priority as u128;
            return ii * ii * ii * ARRAY_INDEX_CUBIC_WEIGHT;
        }
        let bias = self.resolved_bias(arena_id, depth);
        Self::bias_extra(bias, i, kept)
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "Array expansion mixes scoring, arena mapping, and PQ wiring; splitting would obscure the flow."
    )]
    fn expand_array_children(&mut self, entry: &Entry, arena_id: usize) {
        let parent_is_code_array =
            self.arena.nodes[arena_id].array_bias_override.is_some();
        let node = &self.arena.nodes[arena_id];
        let kept = node.children_len;
        for i in 0..kept {
            let child_arena_id = self.arena.children[node.children_start + i];
            let child_kind = self.arena.nodes[child_arena_id].kind;
            let orig_index = if node.arr_indices_len > 0 {
                let start = node.arr_indices_start;
                self.arena.arr_indices[start + i]
            } else {
                i
            };
            let extra = if self
                .zero_bias_for_code_parent(parent_is_code_array, entry)
            {
                0
            } else {
                self.array_extra_for_index(
                    i,
                    kept,
                    Some(arena_id),
                    entry.depth,
                )
            };
            let mut score = entry.score + ARRAY_CHILD_BASE_INCREMENT + extra;
            if self.arena.nodes[child_arena_id].prefers_parent_line {
                score = score.saturating_sub(CODE_PARENT_LINE_BONUS);
            }
            let child_node = &self.arena.nodes[child_arena_id];
            if matches!(
                child_kind,
                NodeKind::Null | NodeKind::Bool | NodeKind::Number
            ) && child_node
                .atomic_token
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(false)
            {
                score = score.saturating_add(CODE_EMPTY_LINE_PENALTY);
            }
            let Some(score) = self.adjust_code_child_score(
                score,
                parent_is_code_array,
                child_kind,
                child_node,
                child_arena_id,
            ) else {
                continue;
            };
            let child_priority_index = *self.next_pq_id;
            *self.next_pq_id += 1;
            let atomic = child_node.atomic_token.clone();
            self.push_child_common(
                entry,
                child_priority_index,
                CommonChild {
                    arena_index: Some(child_arena_id),
                    score,
                    ranked: match child_kind {
                        NodeKind::Array => RankedNode::Array {
                            node_id: NodeId(child_priority_index),
                            key_in_object: None,
                        },
                        NodeKind::Object => RankedNode::Object {
                            node_id: NodeId(child_priority_index),
                            key_in_object: None,
                        },
                        NodeKind::String => RankedNode::SplittableLeaf {
                            node_id: NodeId(child_priority_index),
                            key_in_object: None,
                            value: child_node
                                .string_value
                                .clone()
                                .unwrap_or_default(),
                        },
                        NodeKind::Null | NodeKind::Bool | NodeKind::Number => {
                            RankedNode::AtomicLeaf {
                                node_id: NodeId(child_priority_index),
                                key_in_object: None,
                                token: atomic.unwrap_or_default(),
                            }
                        }
                    },
                    index_in_parent_array: Some(orig_index),
                },
            );
            if *self.next_pq_id >= self.safety_cap {
                break;
            }
        }
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "Object child expansion handles sorting by key, scoring, and PQ wiring in one place for clarity"
    )]
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
            let ka = &self.arena.obj_keys[a.0];
            let kb = &self.arena.obj_keys[b.0];
            match ka.cmp(kb) {
                std::cmp::Ordering::Equal => a.0.cmp(&b.0),
                other => other,
            }
        });
        for (key_idx, child_arena_id) in items {
            let child_kind = self.arena.nodes[child_arena_id].kind;
            let child_priority_index = *self.next_pq_id;
            *self.next_pq_id += 1;
            let score = entry.score + OBJECT_CHILD_BASE_INCREMENT;
            let child_node = &self.arena.nodes[child_arena_id];
            let atomic = child_node.atomic_token.clone();
            self.push_child_common(
                entry,
                child_priority_index,
                CommonChild {
                    arena_index: Some(child_arena_id),
                    score,
                    ranked: match child_kind {
                        NodeKind::Array => RankedNode::Array {
                            node_id: NodeId(child_priority_index),
                            key_in_object: Some(
                                self.arena.obj_keys[key_idx].clone(),
                            ),
                        },
                        NodeKind::Object => RankedNode::Object {
                            node_id: NodeId(child_priority_index),
                            key_in_object: Some(
                                self.arena.obj_keys[key_idx].clone(),
                            ),
                        },
                        NodeKind::String => RankedNode::SplittableLeaf {
                            node_id: NodeId(child_priority_index),
                            key_in_object: Some(
                                self.arena.obj_keys[key_idx].clone(),
                            ),
                            value: child_node
                                .string_value
                                .clone()
                                .unwrap_or_default(),
                        },
                        NodeKind::Null | NodeKind::Bool | NodeKind::Number => {
                            RankedNode::AtomicLeaf {
                                node_id: NodeId(child_priority_index),
                                key_in_object: Some(
                                    self.arena.obj_keys[key_idx].clone(),
                                ),
                                token: atomic.unwrap_or_default(),
                            }
                        }
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
        let full = match &self.nodes[id] {
            RankedNode::SplittableLeaf { value, .. } => value.as_str(),
            // LeafPart (and any non-splittable) should not expand further; treat as empty
            _ => "",
        };
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
                    ranked: RankedNode::LeafPart {
                        node_id: NodeId(child_priority_index),
                        key_in_object: None,
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

fn code_line_length_extreme(token: &str) -> bool {
    let length = token.trim().chars().count();
    !(CODE_SHORT_LINE_THRESHOLD..=CODE_LONG_LINE_THRESHOLD).contains(&length)
}

fn code_line_is_brace_only(token: &str) -> bool {
    let trimmed = token.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|c| matches!(c, '{' | '}' | '(' | ')' | '[' | ']' | ';'))
        && trimmed.contains('}')
}

fn code_array_is_brace_only(arena: &JsonTreeArena, array_id: usize) -> bool {
    let node = &arena.nodes[array_id];
    if node.kind != NodeKind::Array || node.children_len == 0 {
        return false;
    }
    let mut any_atomic = false;
    for idx in 0..node.children_len {
        let child_id = arena.children[node.children_start + idx];
        let child = &arena.nodes[child_id];
        match child.kind {
            NodeKind::Null | NodeKind::Bool | NodeKind::Number => {
                if let Some(token) = child.atomic_token.as_ref() {
                    if !code_line_is_brace_only(token) {
                        return false;
                    }
                    any_atomic = true;
                }
            }
            NodeKind::Array => {
                if !code_array_is_brace_only(arena, child_id) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    any_atomic
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "Orchestrates the full build; further splitting would decrease readability"
)]
pub fn build_order(
    arena: &JsonTreeArena,
    config: &PriorityConfig,
) -> Result<PriorityOrder> {
    let fileset_slots = compute_fileset_slots(arena);
    let duplicate_counts =
        compute_duplicate_line_counts(arena, fileset_slots.as_deref());
    let mut next_pq_id: usize = 0;
    let mut nodes: Vec<RankedNode> = Vec::new();
    let mut scores: Vec<u128> = Vec::new();
    let mut parent: Vec<Option<NodeId>> = Vec::new();
    let mut children: Vec<Vec<NodeId>> = Vec::new();
    let mut metrics: Vec<NodeMetrics> = Vec::new();
    let mut order: Vec<NodeId> = Vec::new();
    let mut object_type: Vec<ObjectType> = Vec::new();
    let mut heap: BinaryHeap<Reverse<Entry>> = BinaryHeap::new();
    let mut index_in_parent_array: Vec<Option<usize>> = Vec::new();
    let mut arena_to_pq: Vec<Option<usize>> = vec![None; arena.nodes.len()];
    let mut node_slots: Vec<Option<usize>> = Vec::new();

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
    let root_atomic = n.atomic_token.clone();
    let root_node = match root_kind {
        NodeKind::Array => RankedNode::Array {
            node_id: NodeId(root_priority_index),
            key_in_object: None,
        },
        NodeKind::Object => RankedNode::Object {
            node_id: NodeId(root_priority_index),
            key_in_object: None,
        },
        NodeKind::String => RankedNode::SplittableLeaf {
            node_id: NodeId(root_priority_index),
            key_in_object: None,
            value: n.string_value.clone().unwrap_or_default(),
        },
        NodeKind::Null | NodeKind::Bool | NodeKind::Number => {
            RankedNode::AtomicLeaf {
                node_id: NodeId(root_priority_index),
                key_in_object: None,
                token: root_atomic.unwrap_or_default(),
            }
        }
    };
    nodes.push(root_node);
    scores.push(ROOT_BASE_SCORE);
    // Root object type: mark fileset root specially, otherwise Object.
    let root_ot = if arena.is_fileset {
        ObjectType::Fileset
    } else {
        ObjectType::Object
    };
    object_type.push(root_ot);
    if root_ar >= arena_to_pq.len() {
        arena_to_pq.resize(root_ar + 1, None);
    }
    arena_to_pq[root_ar] = Some(root_priority_index);
    node_slots.push(None);
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
            scores: &mut scores,
            heap: &mut heap,
            safety_cap: SAFETY_CAP,
            object_type: &mut object_type,
            index_in_parent_array: &mut index_in_parent_array,
            arena_to_pq: &mut arena_to_pq,
            node_slots: &mut node_slots,
            arena_slots: fileset_slots.as_deref(),
            duplicate_counts: &duplicate_counts,
        };
        scope.process_entry(&entry, &mut order);
        if next_pq_id >= SAFETY_CAP {
            break;
        }
    }

    let fileset_children = if arena.is_fileset {
        let root = &arena.nodes[arena.root_id];
        let mut ids: Vec<NodeId> = Vec::with_capacity(root.children_len);
        for idx in 0..root.children_len {
            let child_arena_id = arena.children[root.children_start + idx];
            if let Some(Some(pq_id)) = arena_to_pq.get(child_arena_id) {
                ids.push(NodeId(*pq_id));
            }
        }
        let file_count = root.children_len;
        interleave_fileset_priority(&mut order, &node_slots, file_count, &ids);
        Some(ids)
    } else {
        None
    };

    let total = next_pq_id;
    let mut code_lines: HashMap<usize, Arc<Vec<String>>> = HashMap::new();
    for (arena_idx, lines) in &arena.code_lines {
        if let Some(Some(pq_id)) = arena_to_pq.get(*arena_idx) {
            code_lines.insert(*pq_id, Arc::clone(lines));
        }
    }
    Ok(PriorityOrder {
        metrics,
        nodes,
        scores,
        parent,
        children,
        index_in_parent_array,
        by_priority: order,
        total_nodes: total,
        object_type,
        code_lines,
        fileset_children,
    })
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Counting global and per-file duplicates together keeps slot logic local"
)]
fn compute_duplicate_line_counts(
    arena: &JsonTreeArena,
    slots: Option<&[Option<usize>]>,
) -> DuplicateCounts {
    use std::collections::HashMap;
    let mut global: HashMap<String, usize> = HashMap::new();
    let mut per_file: Option<Vec<HashMap<String, usize>>> = None;
    if let Some(slot_map) = slots {
        let file_count = arena
            .nodes
            .get(arena.root_id)
            .map(|n| n.children_len)
            .unwrap_or(0);
        per_file = Some(vec![HashMap::new(); file_count]);
        for (idx, node) in arena.nodes.iter().enumerate() {
            if let Some(token) = node.atomic_token.as_ref() {
                let norm = token.trim();
                if norm.is_empty() {
                    continue;
                }
                *global.entry(norm.to_string()).or_insert(0) += 1;
                if let Some(slot) = slot_map.get(idx).copied().flatten() {
                    if let Some(vec) = per_file.as_mut() {
                        if let Some(map) = vec.get_mut(slot) {
                            *map.entry(norm.to_string()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    } else {
        for node in &arena.nodes {
            if let Some(token) = node.atomic_token.as_ref() {
                let norm = token.trim();
                if norm.is_empty() {
                    continue;
                }
                *global.entry(norm.to_string()).or_insert(0) += 1;
            }
        }
    }
    DuplicateCounts { global, per_file }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn duplicate_lines_penalized_in_code_mode() {
        let input = b"dup\nunique\ndup\n".to_vec();
        let mut cfg = PriorityConfig::new(usize::MAX, 5);
        cfg.line_budget_only = false;
        let arena = crate::ingest::formats::text::build_text_tree_arena_from_bytes_with_mode(
            input,
            &cfg,
            true,
        )
        .expect("arena");
        let build = super::build_order(&arena, &cfg).expect("order");
        // Collect priority positions for each line token.
        let mut positions = std::collections::HashMap::new();
        for (pos, nid) in build.by_priority.iter().enumerate() {
            if let Some(RankedNode::AtomicLeaf { token, .. }) =
                build.nodes.get(nid.0)
            {
                positions
                    .entry(token.clone())
                    .or_insert_with(Vec::new)
                    .push(pos);
            }
        }
        let unique_pos = positions
            .get("unique")
            .and_then(|v| v.first().copied())
            .unwrap_or(usize::MAX);
        let dup_pos = positions
            .get("dup")
            .and_then(|v| v.first().copied())
            .unwrap_or(usize::MAX);
        assert!(
            unique_pos < dup_pos,
            "expected unique line to outrank duplicate line: unique at {unique_pos}, dup at {dup_pos}"
        );
    }

    #[test]
    fn duplicate_lines_not_penalized_across_fileset() {
        // Two files share "dup"; each has a unique line. Cross-file duplicates should not be demoted.
        let mut cfg = PriorityConfig::new(usize::MAX, 5);
        cfg.line_budget_only = false;
        let arena = crate::ingest::fileset::build_fileset_root(vec![
            (
                "a".to_string(),
                crate::ingest::formats::text::build_text_tree_arena_from_bytes_with_mode(
                    b"dup\nunique_a\n".to_vec(),
                    &cfg,
                    true,
                )
                .expect("arena"),
            ),
            (
                "b".to_string(),
                crate::ingest::formats::text::build_text_tree_arena_from_bytes_with_mode(
                    b"dup\nunique_b\n".to_vec(),
                    &cfg,
                    true,
                )
                .expect("arena"),
            ),
        ]);
        let build = super::build_order(&arena, &cfg).expect("order");
        let mut positions = std::collections::HashMap::new();
        for (pos, nid) in build.by_priority.iter().enumerate() {
            if let Some(RankedNode::AtomicLeaf { token, .. }) =
                build.nodes.get(nid.0)
            {
                positions
                    .entry(token.clone())
                    .or_insert_with(Vec::new)
                    .push(pos);
            }
        }
        let dup_pos = positions
            .get("dup")
            .and_then(|v| v.first().copied())
            .unwrap_or(usize::MAX);
        let unique_pos_a = positions
            .get("unique_a")
            .and_then(|v| v.first().copied())
            .unwrap_or(usize::MAX);
        let unique_pos_b = positions
            .get("unique_b")
            .and_then(|v| v.first().copied())
            .unwrap_or(usize::MAX);
        assert!(
            dup_pos < unique_pos_a && dup_pos < unique_pos_b,
            "expected cross-file duplicate to appear before uniques (no cross-file penalty): dup={dup_pos}, ua={unique_pos_a}, ub={unique_pos_b}"
        );
    }

    #[test]
    fn order_empty_array() {
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
            order_index
                .get(it.node_id().0)
                .copied()
                .unwrap_or(usize::MAX)
        });
        let mut lines = vec![format!("len={}", build.total_nodes)];
        for it in items_sorted {
            lines.push(format!("{it:?}"));
        }
        assert_snapshot!("order_empty_array_order", lines.join("\n"));
    }

    #[test]
    fn order_single_string_array() {
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
            order_index
                .get(it.node_id().0)
                .copied()
                .unwrap_or(usize::MAX)
        });
        let mut lines = vec![format!("len={}", build.total_nodes)];
        for it in items_sorted {
            lines.push(format!("{it:?}"));
        }
        assert_snapshot!("order_single_string_array_order", lines.join("\n"));
    }

    #[test]
    fn code_line_length_extreme_respects_trimmed_bounds() {
        assert!(super::code_line_length_extreme(" hi"));
        assert!(!super::code_line_length_extreme("hello"));
        let long_line = "x".repeat(CODE_LONG_LINE_THRESHOLD + 1);
        assert!(super::code_line_length_extreme(&long_line));
        let mut exact_short = " ".repeat(2);
        exact_short.push_str("12345");
        assert!(!super::code_line_length_extreme(&exact_short));
    }

    #[test]
    fn code_line_is_brace_only_detection() {
        assert!(super::code_line_is_brace_only(" }"));
        assert!(super::code_line_is_brace_only("});"));
        assert!(!super::code_line_is_brace_only("function demo() {"));
    }

    #[test]
    fn code_array_is_brace_only_matches_single_child() {
        use crate::utils::tree_arena::{JsonTreeArena, JsonTreeNode};
        let mut arena = JsonTreeArena::default();
        let brace_child = JsonTreeNode {
            kind: NodeKind::Number,
            atomic_token: Some("}".to_string()),
            ..JsonTreeNode::default()
        };
        arena.nodes.push(brace_child);
        let child_id = 0usize;
        let children_start = arena.children.len();
        arena.children.push(child_id);
        let parent = JsonTreeNode {
            kind: NodeKind::Array,
            children_start,
            children_len: 1,
            ..JsonTreeNode::default()
        };
        arena.nodes.push(parent);
        let array_id = 1usize;
        assert!(super::code_array_is_brace_only(&arena, array_id));
    }
}
