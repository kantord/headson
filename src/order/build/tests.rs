use super::*;
use insta::assert_snapshot;

fn slot_for(
    node: NodeId,
    parent: &[Option<NodeId>],
    fileset_children: &[NodeId],
) -> Option<usize> {
    let mut current = node;
    loop {
        let p = parent.get(current.0).and_then(|p| *p)?;
        if p.0 == ROOT_PQ_ID {
            return fileset_children.iter().position(|fid| *fid == current);
        }
        current = p;
    }
}

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
fn fileset_round_robin_with_duplicates_and_braces() {
    let mut cfg = PriorityConfig::new(usize::MAX, 8);
    cfg.line_budget_only = true;
    let arena = crate::ingest::fileset::build_fileset_root(vec![
        (
            "a.rs".to_string(),
            crate::ingest::formats::text::build_text_tree_arena_from_bytes_with_mode(
                b"fn shared() {}\n{\n}\nshared()\n".to_vec(),
                &cfg,
                true,
            )
            .expect("arena"),
        ),
        (
            "b.rs".to_string(),
            crate::ingest::formats::text::build_text_tree_arena_from_bytes_with_mode(
                b"fn shared() {}\n{\n}\nunique_b()\n".to_vec(),
                &cfg,
                true,
            )
            .expect("arena"),
        ),
    ]);
    let build = super::build_order(&arena, &cfg).expect("order");
    let files = build.fileset_children.as_ref().expect("fileset roots");
    let mut lines: Vec<String> = Vec::new();
    for nid in &build.by_priority {
        let Some(slot) = slot_for(*nid, &build.parent, files) else {
            continue;
        };
        if let Some(RankedNode::AtomicLeaf { token, .. }) =
            build.nodes.get(nid.0)
        {
            lines.push(format!("f{slot}:{}", token.trim()));
        }
    }
    // Limit the snapshot to the top portion to keep it readable.
    let head: Vec<_> = lines.into_iter().take(12).collect();
    assert_snapshot!(
        "fileset_round_robin_with_duplicates_and_braces",
        head.join("\n")
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
