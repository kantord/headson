use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::RankedNode;
use crate::order::{NodeId, PriorityOrder};

fn is_under_code_lines(order: &PriorityOrder, node_id: NodeId) -> bool {
    let mut cursor = node_id;
    loop {
        if order.code_lines.contains_key(&cursor.0) {
            return true;
        }
        match order.parent.get(cursor.0).and_then(|p| *p) {
            Some(parent) => cursor = parent,
            None => return false,
        }
    }
}

fn build_json_path(order: &PriorityOrder, node_id: NodeId) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut cursor = node_id;
    while let Some(parent) = order.parent.get(cursor.0).and_then(|p| *p) {
        if let Some(key) =
            order.nodes.get(cursor.0).and_then(|n| n.key_in_object())
        {
            parts.push(key.to_string());
        } else if let Some(idx) =
            order.index_in_parent_array.get(cursor.0).and_then(|x| *x)
        {
            parts.push(idx.to_string());
        }
        cursor = parent;
    }
    parts.reverse();
    parts.join(".")
}

/// Returns `(file, path)` for a leaf node, or `None` for structural nodes.
///
/// `file` is always `""` for single-file inputs.
/// `path` is a dot-joined key/index chain (JSON/YAML) or a hex hash of the
/// trimmed token content (code mode — any ancestor is in `order.code_lines`).
pub fn leaf_breadcrumb_key(
    order: &PriorityOrder,
    node_id: NodeId,
) -> Option<(String, String)> {
    let node = order.nodes.get(node_id.0)?;
    let token_str: &str = match node {
        RankedNode::Array { .. }
        | RankedNode::Object { .. }
        | RankedNode::LeafPart { .. } => return None,
        RankedNode::AtomicLeaf { token, .. } => token.as_str(),
        RankedNode::SplittableLeaf { value, .. } => value.as_str(),
    };
    let path = if is_under_code_lines(order, node_id) {
        let mut h = DefaultHasher::new();
        token_str.trim().hash(&mut h);
        format!("{:x}", h.finish())
    } else {
        build_json_path(order, node_id)
    };
    Some((String::new(), path))
}

pub(crate) fn collect_shown_leaves(
    order: &PriorityOrder,
    top_k: usize,
) -> Vec<(String, String)> {
    let bound = top_k.min(order.by_priority.len());
    order.by_priority[..bound]
        .iter()
        .filter_map(|&node_id| leaf_breadcrumb_key(order, node_id))
        .collect()
}
