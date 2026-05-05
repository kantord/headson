use crate::RankedNode;
use crate::order::{NodeId, PriorityOrder};

const FNV1A_INIT: u64 = 14_695_981_039_346_656_037;
const FNV1A_PRIME: u64 = 1_099_511_628_211;

fn fnv1a_update(mut h: u64, data: &[u8]) -> u64 {
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(FNV1A_PRIME);
    }
    h
}

fn fnv1a(data: &[u8]) -> u64 {
    fnv1a_update(FNV1A_INIT, data)
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

fn node_hash(order: &PriorityOrder, id: usize, hashes: &[u64]) -> u64 {
    match order.nodes.get(id) {
        None => 0,
        Some(RankedNode::AtomicLeaf { token, .. }) => fnv1a(token.as_bytes()),
        Some(RankedNode::SplittableLeaf { value, .. }) => fnv1a(value.as_bytes()),
        Some(RankedNode::LeafPart { .. }) => 0,
        Some(RankedNode::Object { .. } | RankedNode::Array { .. }) => {
            let mut h = FNV1A_INIT;
            if let Some(children) = order.children.get(id) {
                for &child_id in children {
                    if let Some(key) =
                        order.nodes.get(child_id.0).and_then(|n| n.key_in_object())
                    {
                        h = fnv1a_update(h, key.as_bytes());
                    }
                    h = fnv1a_update(
                        h,
                        &hashes.get(child_id.0).copied().unwrap_or(0).to_le_bytes(),
                    );
                }
            }
            h
        }
    }
}

/// Compute a stable FNV-1a Merkle hash for every node in `order`.
///
/// Hashes are built bottom-up:
/// - `AtomicLeaf` / `SplittableLeaf`: hash of token/value bytes.
/// - `Object`: hash of (key_bytes ++ child_hash) for each child in order.
/// - `Array`: hash of child hashes in order.
/// - `LeafPart`: 0 (synthetic split-rendering node; excluded from breadcrumbs).
///
/// The returned `Vec<u64>` is indexed by PQ node id (`NodeId.0`).
/// Output is deterministic across process restarts.
pub fn compute_merkle_hashes(order: &PriorityOrder) -> Vec<u64> {
    let n = order.nodes.len();
    let mut hashes = vec![0u64; n];
    if n == 0 {
        return hashes;
    }
    // Root is always PQ id 0 by construction in build_order.
    let mut stack: Vec<(usize, bool)> = Vec::with_capacity(n);
    stack.push((0, false));
    while let Some((id, processed)) = stack.pop() {
        if processed {
            hashes[id] = node_hash(order, id, &hashes);
        } else {
            stack.push((id, true));
            if let Some(children) = order.children.get(id) {
                for &child in children.iter().rev() {
                    stack.push((child.0, false));
                }
            }
        }
    }
    hashes
}

/// Returns `(file, path)` for a leaf node, or `None` for structural nodes.
///
/// `file` is always `""` for single-file inputs; for filesets the filename
/// is embedded in the dot-path via the fileset's synthetic root.
///
/// `path` is `"dot.path#<16 hex digits>"`: the structural address combined
/// with the FNV-1a Merkle hash of the subtree at that node. The composite key
/// is stable across restarts. A content change produces a new hash (no match);
/// reverting the change restores the original hash (penalty re-activates).
pub fn leaf_breadcrumb_key(
    order: &PriorityOrder,
    node_id: NodeId,
    hashes: &[u64],
) -> Option<(String, String)> {
    match order.nodes.get(node_id.0)? {
        RankedNode::Array { .. }
        | RankedNode::Object { .. }
        | RankedNode::LeafPart { .. } => None,
        RankedNode::AtomicLeaf { .. } | RankedNode::SplittableLeaf { .. } => {
            let dot_path = build_json_path(order, node_id);
            let hash = hashes.get(node_id.0).copied().unwrap_or(0);
            Some((String::new(), format!("{dot_path}#{hash:016x}")))
        }
    }
}

pub(crate) fn collect_shown_leaves(
    order: &PriorityOrder,
    top_k: usize,
) -> Vec<(String, String)> {
    let hashes = compute_merkle_hashes(order);
    let bound = top_k.min(order.by_priority.len());
    order.by_priority[..bound]
        .iter()
        .filter_map(|&node_id| leaf_breadcrumb_key(order, node_id, &hashes))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::PriorityConfig;
    use crate::ingest::{parse_json_one, parse_text_one_with_mode};
    use crate::order::build_order;

    fn make_order(json: &[u8]) -> PriorityOrder {
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let arena = parse_json_one(json.to_vec(), &cfg).expect("parse must succeed");
        build_order(&arena, &cfg).expect("build_order must succeed")
    }

    // A. compute_merkle_hashes is stable across two separate build_order calls
    // for the same JSON bytes.
    #[test]
    fn merkle_hashes_stable_across_builds() {
        let json = b"{\"x\": 42, \"y\": \"hello\"}";
        let order1 = make_order(json);
        let order2 = make_order(json);
        let hashes1 = compute_merkle_hashes(&order1);
        let hashes2 = compute_merkle_hashes(&order2);
        assert_eq!(hashes1, hashes2,
            "Merkle hashes must be identical across two builds of the same input");
    }

    // B. Root-node hash differs when a leaf value changes.
    #[test]
    fn merkle_hash_changes_when_leaf_value_changes() {
        let order_a = make_order(b"{\"a\": 1}");
        let order_b = make_order(b"{\"a\": 2}");
        let hashes_a = compute_merkle_hashes(&order_a);
        let hashes_b = compute_merkle_hashes(&order_b);
        // Root is node 0 in PriorityOrder convention.
        assert_ne!(
            hashes_a[crate::order::types::ROOT_PQ_ID],
            hashes_b[crate::order::types::ROOT_PQ_ID],
            "Root hash must differ when leaf value changes"
        );
    }

    // C. The hash of a leaf that did NOT change is unaffected by a sibling change.
    #[test]
    fn merkle_hash_sibling_unchanged_when_other_sibling_changes() {
        // {"a": 1, "b": "hello"} vs {"a": 1, "b": "world"}
        // The leaf for value 1 (key "a") must hash identically in both.
        let order_hello = make_order(b"{\"a\": 1, \"b\": \"hello\"}");
        let order_world = make_order(b"{\"a\": 1, \"b\": \"world\"}");
        let hashes_hello = compute_merkle_hashes(&order_hello);
        let hashes_world = compute_merkle_hashes(&order_world);

        // Find the node id for the leaf value of key "a" (AtomicLeaf with token "1")
        // in each order.
        let find_a_leaf_id = |order: &PriorityOrder| -> usize {
            order.nodes.iter().position(|n| {
                matches!(n, RankedNode::AtomicLeaf { token, key_in_object, .. }
                    if token == "1" && key_in_object.as_deref() == Some("a"))
            }).expect("must find leaf for key 'a' with value 1")
        };

        let id_hello = find_a_leaf_id(&order_hello);
        let id_world = find_a_leaf_id(&order_world);

        assert_eq!(
            hashes_hello[id_hello],
            hashes_world[id_world],
            "Hash for unchanged leaf ('a': 1) must be the same regardless of sibling change"
        );
    }

    // D. The composite key format for a leaf contains '#' separating dot_path
    // from a 16-hex-char hash string.
    #[test]
    fn composite_key_format_contains_hash_separator() {
        let json = b"{\"name\": \"alice\"}";
        let order = make_order(json);
        let hashes = compute_merkle_hashes(&order);

        // Find the SplittableLeaf for value "alice"
        let alice_id = order.nodes.iter().position(|n| {
            matches!(n, RankedNode::SplittableLeaf { value, key_in_object, .. }
                if value == "alice" && key_in_object.as_deref() == Some("name"))
        }).expect("must find 'alice' leaf");

        let result = leaf_breadcrumb_key(&order, NodeId(alice_id), &hashes);
        let (_, path) = result.expect("leaf_breadcrumb_key must return Some for a leaf");

        let parts: Vec<&str> = path.splitn(2, '#').collect();
        assert_eq!(parts.len(), 2, "path must contain exactly one '#' separator; got: {path:?}");
        assert_eq!(parts[0], "name", "dot_path before '#' must be 'name'; got: {path:?}");
        assert_eq!(parts[1].len(), 16, "hex hash after '#' must be 16 chars; got: {path:?}");
        assert!(
            parts[1].chars().all(|c| c.is_ascii_hexdigit()),
            "hash part must be hex digits; got: {path:?}"
        );
    }

    // E. The composite key is stable across two separate builds of the same input.
    #[test]
    fn composite_key_stable_across_builds() {
        let json = b"{\"k\": \"v\"}";
        let order1 = make_order(json);
        let order2 = make_order(json);
        let hashes1 = compute_merkle_hashes(&order1);
        let hashes2 = compute_merkle_hashes(&order2);

        // Find the leaf in each order.
        let find_leaf = |order: &PriorityOrder| -> NodeId {
            NodeId(order.nodes.iter().position(|n| {
                matches!(n, RankedNode::SplittableLeaf { value, .. } if value == "v")
            }).expect("must find leaf 'v'"))
        };

        let key1 = leaf_breadcrumb_key(&order1, find_leaf(&order1), &hashes1);
        let key2 = leaf_breadcrumb_key(&order2, find_leaf(&order2), &hashes2);
        assert_eq!(key1, key2, "composite key must be identical across two builds of the same input");
    }

    // F. Composite key for unchanged leaf is stable when a sibling leaf changes.
    #[test]
    fn composite_key_stable_when_sibling_changes() {
        let order_hello = make_order(b"{\"a\": 1, \"b\": \"hello\"}");
        let order_world = make_order(b"{\"a\": 1, \"b\": \"world\"}");
        let hashes_hello = compute_merkle_hashes(&order_hello);
        let hashes_world = compute_merkle_hashes(&order_world);

        let find_a = |order: &PriorityOrder| -> NodeId {
            NodeId(order.nodes.iter().position(|n| {
                matches!(n, RankedNode::AtomicLeaf { token, key_in_object, .. }
                    if token == "1" && key_in_object.as_deref() == Some("a"))
            }).expect("must find AtomicLeaf for key 'a'"))
        };

        let key_hello = leaf_breadcrumb_key(&order_hello, find_a(&order_hello), &hashes_hello);
        let key_world = leaf_breadcrumb_key(&order_world, find_a(&order_world), &hashes_world);
        assert_eq!(
            key_hello, key_world,
            "composite key for unchanged leaf must be stable when a sibling changes"
        );
    }

    // G. leaf_breadcrumb_key returns None for all Array and Object structural nodes.
    #[test]
    fn structural_nodes_return_none() {
        // {"a": [1, 2]} — contains a root Object and one Array child.
        let order = make_order(b"{\"a\": [1, 2]}");
        let hashes = compute_merkle_hashes(&order);

        for (idx, node) in order.nodes.iter().enumerate() {
            if matches!(node, RankedNode::Array { .. } | RankedNode::Object { .. }) {
                let result = leaf_breadcrumb_key(&order, NodeId(idx), &hashes);
                assert!(
                    result.is_none(),
                    "leaf_breadcrumb_key must return None for structural node at index {idx}: {node:?}"
                );
            }
        }
    }

    // H. Code mode: AtomicLeaf composite key path contains '#' and the dot_path
    // part (before '#') is NOT a standalone hex string (old behavior was to
    // return only a hash with no dot-path prefix).
    #[test]
    fn code_mode_no_longer_special_cased() {
        let code = b"fn foo() {\n    let x = 1;\n}\n";
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let arena = parse_text_one_with_mode(code.to_vec(), &cfg, true)
            .expect("parse must succeed");
        let order = build_order(&arena, &cfg).expect("build_order must succeed");
        let hashes = compute_merkle_hashes(&order);

        let mut found_leaf = false;
        for (idx, node) in order.nodes.iter().enumerate() {
            if matches!(node, RankedNode::AtomicLeaf { .. }) {
                if let Some((_, path)) = leaf_breadcrumb_key(&order, NodeId(idx), &hashes) {
                    found_leaf = true;
                    assert!(
                        path.contains('#'),
                        "code-mode leaf path must contain '#'; got: {path:?}"
                    );
                    let dot_path = path.split('#').next().unwrap_or("");
                    // Old behavior was a bare hex hash with no dot_path prefix.
                    // The dot_path must NOT be a purely hex string (or must be non-empty
                    // with non-hex characters like digits-only index or a line label).
                    // Specifically, it must not look like a standalone 16-hex-char hash.
                    let is_standalone_hex_hash = dot_path.len() >= 8
                        && dot_path.chars().all(|c| c.is_ascii_hexdigit());
                    assert!(
                        !is_standalone_hex_hash,
                        "dot_path before '#' must NOT be a standalone hex hash; got dot_path={dot_path:?} in path={path:?}"
                    );
                }
            }
        }
        assert!(found_leaf, "must have found at least one AtomicLeaf in code input");
    }
}
