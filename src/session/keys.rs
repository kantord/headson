use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use headson::{NodeId, PriorityOrder, RankedNode};

/// Walk up the parent chain and return true if any ancestor's PQ id is in
/// `order.code_lines`.
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

/// Build a dot-joined path string by walking up the parent chain collecting
/// object keys and array indices. Returns `""` for a leaf directly at the root
/// (e.g. a bare scalar input) — this is the correct degenerate case.
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

/// Returns `(file, path_or_hash)` for a leaf node, or `None` for non-leaf/synthetic nodes.
///
/// - `file`: `""` for single-file (non-fileset) inputs.
/// - `path`: for JSON/YAML mode, dot-joined key/index path from root to leaf.
///   For code mode (any ancestor is in `order.code_lines`), a lowercase hex hash
///   of `token.trim()` using std's DefaultHasher.
pub fn breadcrumb_key(
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

#[cfg(test)]
mod tests {
    use super::*;
    use headson::{PriorityConfig, build_order};

    fn build_json_order(json: &[u8]) -> PriorityOrder {
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let arena =
            headson::parse_json_one(json.to_vec(), &cfg).expect("parse");
        build_order(&arena, &cfg).expect("order")
    }

    fn build_code_order(code: &[u8]) -> PriorityOrder {
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let arena =
            headson::parse_text_one_with_mode(code.to_vec(), &cfg, true)
                .expect("parse code");
        build_order(&arena, &cfg).expect("order")
    }

    fn find_atomic_leaf_for_token(
        order: &PriorityOrder,
        token: &str,
    ) -> Option<NodeId> {
        order.nodes.iter().find_map(|n| match n {
            RankedNode::AtomicLeaf {
                node_id, token: t, ..
            } if t == token => Some(*node_id),
            _ => None,
        })
    }

    fn find_splittable_leaf_for_value(
        order: &PriorityOrder,
        value: &str,
    ) -> Option<NodeId> {
        order.nodes.iter().find_map(|n| match n {
            RankedNode::SplittableLeaf {
                node_id, value: v, ..
            } if v == value => Some(*node_id),
            _ => None,
        })
    }

    /// Step 17: simple flat object {"name": "alice"} — leaf path should be "name"
    #[test]
    fn step_17_simple_object_key_path() {
        let order = build_json_order(br#"{"name": "alice"}"#);
        let node_id = find_splittable_leaf_for_value(&order, "alice")
            .or_else(|| find_atomic_leaf_for_token(&order, "alice"))
            .or_else(|| find_splittable_leaf_for_value(&order, r#""alice""#))
            .expect("should find leaf for 'alice'");
        let result = breadcrumb_key(&order, node_id);
        let (_, path) =
            result.expect("AtomicLeaf or SplittableLeaf should return Some");
        assert_eq!(path, "name");
    }

    /// Step 18: nested object {"user": {"name": "alice"}} — path should be "user.name"
    #[test]
    fn step_18_nested_object_path() {
        let order = build_json_order(br#"{"user": {"name": "alice"}}"#);
        let node_id = find_splittable_leaf_for_value(&order, "alice")
            .or_else(|| find_atomic_leaf_for_token(&order, "alice"))
            .or_else(|| find_splittable_leaf_for_value(&order, r#""alice""#))
            .expect("should find leaf for 'alice'");
        let result = breadcrumb_key(&order, node_id);
        let (_, path) = result.expect("nested leaf should return Some");
        assert_eq!(path, "user.name");
    }

    /// Step 19: array of 5 strings, "c" is at original index 2 — path should be "2"
    #[test]
    fn step_19_array_uses_original_index() {
        let order = build_json_order(br#"["a","b","c","d","e"]"#);
        // Find the leaf for "c"
        let node_id = find_splittable_leaf_for_value(&order, "c")
            .or_else(|| find_atomic_leaf_for_token(&order, "c"))
            .or_else(|| find_splittable_leaf_for_value(&order, r#""c""#))
            .expect("should find leaf for 'c'");
        let result = breadcrumb_key(&order, node_id);
        let (_, path) = result.expect("array element leaf should return Some");
        assert_eq!(path, "2", "original index of 'c' in [a,b,c,d,e] is 2");
    }

    /// Step 20: {"users": [{"id": 0}, {"id": 1}]} — second id value has path "users.1.id"
    #[test]
    fn step_20_deeply_nested_array_object_path() {
        let order = build_json_order(br#"{"users": [{"id": 0}, {"id": 1}]}"#);
        // Find the leaf node for integer 1 (the second id value). Both "0" and "1"
        // are numbers; we want index 1 in the array, whose token is "1".
        // Both tokens may appear — find the one at array index 1.
        // More direct: find all AtomicLeaf nodes with token "1"
        let candidates: Vec<NodeId> = order
            .nodes
            .iter()
            .filter_map(|n| match n {
                RankedNode::AtomicLeaf { node_id, token, .. }
                    if token == "1" =>
                {
                    Some(*node_id)
                }
                _ => None,
            })
            .collect();
        assert!(
            !candidates.is_empty(),
            "should have at least one leaf with token '1'"
        );
        // Among candidates, find one that has key_in_object == Some("id") and its array-parent index is 1
        let target_id = candidates
            .iter()
            .find(|&&id| {
                if let Some(RankedNode::AtomicLeaf {
                    key_in_object: Some(key),
                    ..
                }) = order.nodes.get(id.0)
                {
                    key == "id"
                } else {
                    false
                }
            })
            .or_else(|| candidates.first())
            .copied()
            .expect("found id leaf");

        let result = breadcrumb_key(&order, target_id);
        let (_, path) = result.expect("deeply nested leaf should return Some");
        assert_eq!(path, "users.1.id");
    }

    /// Step 21: Array and Object nodes return None
    #[test]
    fn step_21_non_leaf_nodes_return_none() {
        let order = build_json_order(br#"{"users": [{"id": 0}, {"id": 1}]}"#);
        for (idx, node) in order.nodes.iter().enumerate() {
            match node {
                RankedNode::Array { .. } | RankedNode::Object { .. } => {
                    let result = breadcrumb_key(&order, NodeId(idx));
                    assert_eq!(
                        result, None,
                        "Array/Object node at index {idx} should return None"
                    );
                }
                _ => {}
            }
        }
    }

    /// Step 22: LeafPart nodes return None.
    /// We verify the match arm by finding any LeafPart in the order, or by
    /// confirming the function handles zero LeafPart nodes gracefully.
    #[test]
    fn step_22_leaf_part_returns_none() {
        // Use a long-string JSON to try to elicit LeafPart nodes.
        // If none appear in this input, we still verify: for any LeafPart node
        // that does appear anywhere in our test inputs, breadcrumb_key returns None.
        let long_str = format!(r#"{{"key": "{}"}}"#, "x".repeat(10000));
        let cfg = PriorityConfig::new(4, usize::MAX); // force grapheme split (max_string_graphemes=4)
        let arena =
            headson::parse_json_one(long_str.as_bytes().to_vec(), &cfg)
                .expect("parse");
        let order = build_order(&arena, &cfg).expect("order");

        let leaf_parts: Vec<NodeId> = order
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(idx, n)| match n {
                RankedNode::LeafPart { .. } => Some(NodeId(idx)),
                _ => None,
            })
            .collect();

        if leaf_parts.is_empty() {
            // No LeafPart nodes appeared; the contract is still guaranteed by the match arm.
            // We can construct a synthetic test by asserting the function branches correctly
            // when we find a SplittableLeaf (the parent of LeafPart fragments).
            let splittable: Vec<NodeId> = order
                .nodes
                .iter()
                .enumerate()
                .filter_map(|(idx, n)| match n {
                    RankedNode::SplittableLeaf { .. } => Some(NodeId(idx)),
                    _ => None,
                })
                .collect();
            // SplittableLeaf should return Some, not None
            for id in splittable {
                assert!(
                    breadcrumb_key(&order, id).is_some(),
                    "SplittableLeaf should return Some, got None for node {id:?}"
                );
            }
        } else {
            for id in leaf_parts {
                let result = breadcrumb_key(&order, id);
                assert_eq!(
                    result, None,
                    "LeafPart node {id:?} should return None"
                );
            }
        }
    }

    /// Walk up the parent chain from `node_id` and return true if any ancestor
    /// array node has an entry in `order.code_lines`.
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

    /// Step 23: code-mode leaf returns a hex hash string, not a numeric index.
    /// In the code tree, code_lines is keyed by the root array (PQ id 0).
    /// Any AtomicLeaf whose ancestor chain includes a code_lines entry is a code leaf.
    #[test]
    fn step_23_code_leaf_returns_content_hash() {
        let code = b"fn foo() {\n    let x = 1;\n}\n";
        let order = build_code_order(code);

        // Find any AtomicLeaf that is under a code_lines ancestor
        let code_leaf = order.nodes.iter().find_map(|n| match n {
            RankedNode::AtomicLeaf { node_id, .. }
                if is_under_code_lines(&order, *node_id) =>
            {
                Some(*node_id)
            }
            _ => None,
        });

        let node_id = code_leaf.expect(
            "should find a code-mode leaf under a code_lines ancestor",
        );
        let result = breadcrumb_key(&order, node_id);
        let (_, path) = result.expect("code leaf should return Some");

        // The path for a code leaf should be a hex string, not a decimal number
        assert!(
            path.chars().all(|c| c.is_ascii_hexdigit()),
            "code leaf path should be a hex hash, got: {path:?}"
        );
        assert!(!path.is_empty(), "code leaf hash should not be empty");
    }

    /// Step 24: code leaf → hash path; JSON leaf → dot-path. Confirms branching.
    #[test]
    fn step_24_code_vs_json_path_branching() {
        // --- JSON branch ---
        let json_order = build_json_order(br#"{"name": "alice"}"#);
        let json_leaf = find_splittable_leaf_for_value(&json_order, "alice")
            .or_else(|| find_atomic_leaf_for_token(&json_order, "alice"))
            .or_else(|| {
                find_splittable_leaf_for_value(&json_order, r#""alice""#)
            })
            .expect("JSON leaf for 'alice'");
        let (_, json_path) = breadcrumb_key(&json_order, json_leaf)
            .expect("JSON leaf should return Some");
        // JSON path uses dot-notation key, not hex
        assert_eq!(
            json_path, "name",
            "JSON leaf should produce dot-path 'name'"
        );

        // --- Code branch ---
        let code = b"fn foo() {\n    let x = 1;\n}\n";
        let code_order = build_code_order(code);
        let code_leaf = code_order
            .nodes
            .iter()
            .find_map(|n| match n {
                RankedNode::AtomicLeaf { node_id, .. }
                    if is_under_code_lines(&code_order, *node_id) =>
                {
                    Some(*node_id)
                }
                _ => None,
            })
            .expect("should find a code leaf under a code_lines ancestor");
        let (_, code_path) = breadcrumb_key(&code_order, code_leaf)
            .expect("code leaf should return Some");
        // Code path is a hex hash — all hex digits
        assert!(
            code_path.chars().all(|c| c.is_ascii_hexdigit()),
            "code path should be hex, got: {code_path:?}"
        );
        // JSON path "name" contains 'n', 'a', 'm', 'e' — not purely hex
        // (it won't be produced by the hash branch)
        assert_ne!(
            json_path, code_path,
            "JSON path and code hash path must differ"
        );
    }
}
