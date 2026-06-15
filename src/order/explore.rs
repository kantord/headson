use std::collections::HashMap;

use crate::node_path;
use crate::order::types::{
    Breadcrumb, ExploreContext, NodeId, PriorityOrder, RankedNode,
    novelty_penalty,
};

/// Scales the raw novelty penalty (`ln(1 + count) * alpha^steps_ago`) into
/// `u128` score space. Score anchors: object/string siblings differ by ~1;
/// array siblings by `d^3 * 1e12` where `d` is distance to the nearest
/// sampling anchor. At 1e15 a leaf seen once on the previous step (scaled
/// ≈ 3.47e14) outranks unseen siblings up to `d ≈ 7`, shrinking to `d ≈ 4`
/// after three steps at alpha=0.5. The penalty reorders, never excludes.
const PENALTY_SCALE: f64 = 1_000_000_000_000_000.0;

/// Apply the explore novelty penalty to previously-seen leaves and re-sort
/// `by_priority` by the adjusted scores.
///
/// Must run BEFORE fileset interleaving (`build_order` guarantees this): the
/// re-sort restores raw score order, which would otherwise destroy the
/// round-robin fairness applied by `interleave_fileset_priority`.
///
/// The Merkle hash table computed for breadcrumb matching is stored on
/// `order.merkle_hashes` so shown-leaf collection can reuse it without a
/// second full-tree hashing pass.
pub(crate) fn apply_explore_penalty(
    order: &mut PriorityOrder,
    ctx: &ExploreContext,
) {
    if ctx.breadcrumbs.is_empty() {
        return;
    }
    let hashes = node_path::compute_merkle_hashes(order);
    let files = node_path::NodeFiles::for_order(order, ctx.file.as_deref());
    let by_key: HashMap<(&str, &str), &Breadcrumb> = ctx
        .breadcrumbs
        .iter()
        .map(|b| ((b.file.as_str(), b.path.as_str()), b))
        .collect();
    let penalties: Vec<(NodeId, u128)> = order
        .by_priority
        .iter()
        .filter_map(|&node_id| {
            let (file, path) = node_path::leaf_breadcrumb_key(
                order, node_id, &hashes, &files,
            )?;
            let bc = by_key.get(&(file.as_str(), path.as_str()))?;
            let steps_ago = ctx.current_step.saturating_sub(bc.last_step);
            let penalty = novelty_penalty(bc.count, steps_ago, ctx.alpha);
            (penalty > 0.0)
                .then_some((node_id, (penalty * PENALTY_SCALE) as u128))
        })
        .collect();
    if !penalties.is_empty() {
        // Phase 1: bump leaf scores.
        let mut leaf_penalties: Vec<u128> = vec![0; order.total_nodes];
        for &(node_id, delta) in &penalties {
            leaf_penalties[node_id.0] = delta;
            bump_score(order, node_id, delta);
        }
        // Phase 4: propagate mean child penalty upward through structural nodes
        // so that heavily-seen subtrees (e.g. Cargo.lock) lose budget priority
        // relative to less-explored peers, not just individual leaves.
        propagate_penalties_upward(order, &leaf_penalties);
        order.by_priority.sort_by_key(|id| order.scores[id.0]);
    }
    order.merkle_hashes = Some(hashes);
}

/// Propagate leaf penalties upward through the priority tree.
///
/// For each structural node (Array/Object), applies a penalty equal to the
/// mean of its direct children's penalties. This makes heavily-explored
/// subtrees lose budget priority relative to untouched peers, not just
/// individual leaves. One pass over all nodes suffices because the tree is
/// acyclic and we only read leaf penalties (not yet-accumulated parent ones).
fn propagate_penalties_upward(
    order: &mut PriorityOrder,
    leaf_penalties: &[u128],
) {
    let (child_sum, child_cnt) =
        accumulate_child_penalties(order, leaf_penalties);
    apply_mean_penalties(order, &child_sum, &child_cnt);
}

fn accumulate_child_penalties(
    order: &PriorityOrder,
    leaf_penalties: &[u128],
) -> (Vec<u128>, Vec<u32>) {
    let n = order.total_nodes;
    let mut child_sum: Vec<u128> = vec![0; n];
    let mut child_cnt: Vec<u32> = vec![0; n];
    for (pq_idx, &penalty) in leaf_penalties.iter().enumerate().take(n) {
        if penalty == 0 {
            continue;
        }
        if let Some(parent_id) = order.parent[pq_idx] {
            child_sum[parent_id.0] =
                child_sum[parent_id.0].saturating_add(penalty);
            child_cnt[parent_id.0] += 1;
        }
    }
    (child_sum, child_cnt)
}

fn apply_mean_penalties(
    order: &mut PriorityOrder,
    child_sum: &[u128],
    child_cnt: &[u32],
) {
    for (pq_idx, (&sum, &cnt)) in
        child_sum.iter().zip(child_cnt.iter()).enumerate()
    {
        if cnt == 0 {
            continue;
        }
        let mean = sum / u128::from(cnt);
        if mean > 0 {
            order.scores[pq_idx] = order.scores[pq_idx].saturating_add(mean);
        }
    }
}

fn bump_score(order: &mut PriorityOrder, node_id: NodeId, delta: u128) {
    order.scores[node_id.0] = order.scores[node_id.0].saturating_add(delta);
    // Carry the penalty onto the LeafPart children of a penalized
    // SplittableLeaf so the parts cannot sort ahead of their parent;
    // otherwise a part inside the top-k prefix would render the string while
    // the parent leaf goes unrecorded as shown.
    if matches!(order.nodes[node_id.0], RankedNode::SplittableLeaf { .. }) {
        for i in 0..order.children[node_id.0].len() {
            let child = order.children[node_id.0][i];
            order.scores[child.0] =
                order.scores[child.0].saturating_add(delta);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::parse_json_one;
    use crate::order::{PriorityConfig, build_order};

    fn make_order(json: &[u8]) -> PriorityOrder {
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let arena = parse_json_one(json.to_vec(), &cfg).expect("parse");
        build_order(&arena, &cfg).expect("build_order")
    }

    fn key_for(order: &PriorityOrder, dot_path: &str) -> (String, String) {
        let hashes = node_path::compute_merkle_hashes(order);
        let files = node_path::NodeFiles::for_order(order, None);
        order
            .by_priority
            .iter()
            .find_map(|&node_id| {
                let key = node_path::leaf_breadcrumb_key(
                    order, node_id, &hashes, &files,
                )?;
                let prefix = key.1.split_once('#').map(|(p, _)| p)?;
                (prefix == dot_path).then_some(key)
            })
            .unwrap_or_else(|| panic!("no leaf at dot_path {dot_path:?}"))
    }

    #[test]
    fn empty_breadcrumbs_leave_order_and_hashes_untouched() {
        let mut order = make_order(b"{\"a\": 1, \"b\": 2}");
        let before = order.by_priority.clone();
        let ctx = ExploreContext {
            breadcrumbs: vec![],
            current_step: 3,
            alpha: 0.5,
            file: None,
        };
        apply_explore_penalty(&mut order, &ctx);
        assert_eq!(order.by_priority, before);
        assert!(order.merkle_hashes.is_none());
    }

    #[test]
    fn penalized_splittable_leaf_stays_before_its_leaf_parts() {
        // Two string values; penalize "alpha" so its SplittableLeaf moves
        // back — its LeafPart children must move back with it.
        let json = b"{\"x\": \"alpha\", \"y\": \"beta\"}";
        let mut order = make_order(json);
        let (file, path) = key_for(&order, "x");
        let ctx = ExploreContext {
            breadcrumbs: vec![Breadcrumb {
                file,
                path,
                count: 5,
                last_step: 1,
            }],
            current_step: 2,
            alpha: 0.9,
            file: None,
        };
        apply_explore_penalty(&mut order, &ctx);

        let leaf_idx = order
            .nodes
            .iter()
            .position(|n| {
                matches!(n, RankedNode::SplittableLeaf { value, .. } if value == "alpha")
            })
            .expect("alpha leaf");
        let pos_of = |id: usize| {
            order
                .by_priority
                .iter()
                .position(|n| n.0 == id)
                .expect("node present in by_priority")
        };
        let leaf_pos = pos_of(leaf_idx);
        for child in &order.children[leaf_idx] {
            assert!(
                pos_of(child.0) > leaf_pos,
                "LeafPart {child:?} sorted before its penalized parent"
            );
        }
        assert!(
            order.merkle_hashes.is_some(),
            "hash table must be stored for reuse by shown-leaf collection"
        );
    }

    #[test]
    fn breadcrumb_for_other_file_does_not_match_identical_content() {
        // The breadcrumb's path matches leaf "x" exactly (same content hash),
        // but it was recorded for a different file — no penalty may apply.
        let json = b"{\"x\": \"alpha\", \"y\": \"beta\"}";
        let mut order = make_order(json);
        let before = order.by_priority.clone();
        let (_, path) = key_for(&order, "x");
        let ctx = ExploreContext {
            breadcrumbs: vec![Breadcrumb {
                file: "/abs/other.json".to_string(),
                path,
                count: 5,
                last_step: 1,
            }],
            current_step: 2,
            alpha: 0.9,
            file: Some("/abs/this.json".to_string()),
        };
        apply_explore_penalty(&mut order, &ctx);
        assert_eq!(
            order.by_priority, before,
            "a breadcrumb from another file must not penalize this file"
        );
    }

    /// Phase 4: when all leaves under a top-level key "a" are penalized, the
    /// Object node for "a" accumulates a mean penalty and sorts AFTER the
    /// unpenalized "b" Object node.
    ///
    /// Without Phase 4, both Object nodes have the same low base score, so
    /// "a" keeps its original position ahead of "b". With Phase 4, "a" gets
    /// a propagated penalty and moves behind "b" in by_priority.
    #[test]
    fn phase4_penalized_section_ranks_below_unpenalized_sibling() {
        // Two top-level objects. Penalize all leaves under "a".
        let json = br#"{"a": {"x": 1, "y": 2}, "b": {"p": 3, "q": 4}}"#;
        let base_order = make_order(json);

        // Collect breadcrumb keys for all leaves under "a".
        let hashes = node_path::compute_merkle_hashes(&base_order);
        let files = node_path::NodeFiles::for_order(&base_order, None);
        let a_breadcrumbs: Vec<Breadcrumb> = base_order
            .by_priority
            .iter()
            .filter_map(|&id| {
                let (file, path) = node_path::leaf_breadcrumb_key(
                    &base_order,
                    id,
                    &hashes,
                    &files,
                )?;
                // Keys under "a" start with "a." in their dot-path.
                let dot_path = path.split_once('#').map(|(p, _)| p)?;
                (dot_path.starts_with("a.")).then_some(Breadcrumb {
                    file,
                    path,
                    count: 3,
                    last_step: 1,
                })
            })
            .collect();
        assert!(!a_breadcrumbs.is_empty(), "must find leaves under 'a'");

        let mut cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        cfg.explore = Some(ExploreContext {
            breadcrumbs: a_breadcrumbs,
            current_step: 2,
            alpha: 0.5,
            file: None,
        });

        // Build via ingest so the explore penalty (including Phase 4) runs.
        use crate::InputKind;
        use crate::grep::GrepConfig;
        use crate::ingest::ingest_into_arena;
        let arena = ingest_into_arena(
            InputKind::Json(json.to_vec()),
            &cfg,
            &GrepConfig::default(),
        )
        .unwrap()
        .arena;
        let order = build_order(&arena, &cfg).unwrap();

        // Find positions of the "a" and "b" Object nodes in by_priority.
        let pos_of_obj = |key: &str| {
            order.by_priority.iter().position(|id| {
                matches!(&order.nodes[id.0],
                    RankedNode::Object { key_in_object: Some(k), .. } if k == key)
            })
        };
        let a_pos = pos_of_obj("a").expect("Object 'a' in by_priority");
        let b_pos = pos_of_obj("b").expect("Object 'b' in by_priority");

        assert!(
            b_pos < a_pos,
            "Object 'b' (pos {b_pos}) must rank ahead of penalized Object 'a' \
             (pos {a_pos}) — Phase 4 must propagate the leaf penalty to the \
             parent Object"
        );
    }
}
