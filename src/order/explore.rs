use std::collections::HashMap;

use crate::node_path;
use crate::order::types::{
    Breadcrumb, ExploreContext, NodeId, PriorityOrder, RankedNode,
    novelty_penalty,
};

/// Scales the raw novelty penalty (`ln(1 + count) * alpha^steps_ago`, range
/// roughly 0–5 in practice) into `u128` score space before adding it to
/// `order.scores` (lower score = higher priority, so the penalty pushes seen
/// leaves back).
///
/// Score-space anchors the penalty competes with:
/// - Object/string siblings differ by ~1 (`OBJECT_CHILD_BASE_INCREMENT`), so
///   any scale well above the tree-depth increments reorders a seen object
///   key behind all of its unseen siblings.
/// - Array/JSONL/code-line siblings differ by `d^3 * ARRAY_INDEX_CUBIC_WEIGHT`
///   (1e12, `order::scoring`), where `d` is the distance to the nearest
///   head/mid/tail sampling anchor. Displacing a seen leaf past `d` unseen
///   sibling indices requires a penalty above `d^3 * 1e12`.
/// - Code-line heuristics (`CODE_BRACE_ONLY_PENALTY`, `CODE_EMPTY_LINE_PENALTY`)
///   sit at 1–4 × 1e12 and stay subordinate: a recently seen line yields even
///   to unseen low-value lines, which is the intended explore behavior.
///
/// At 1e15 (1000 × `ARRAY_INDEX_CUBIC_WEIGHT`) a leaf seen once on the
/// previous step (raw penalty `ln(2) * 0.5` ≈ 0.347, scaled ≈ 3.47e14)
/// outranks unseen siblings up to `d = 7` (7³ = 343 < 347 < 512 = 8³). With
/// the default `alpha = 0.5` the suppression decays to `d ≈ 5` after two
/// steps and `d ≈ 4` after three, so repeated identical invocations explore
/// outward in rings of ~5–8 fresh indices around each sampling anchor before
/// decayed items re-enter; `alpha` → 1 keeps the rings expanding instead.
///
/// Empirical tuning (issue #513 end-of-Phase-1 checkpoint; 100-element array
/// and 100-line JSONL fixtures at `-c 200..1000`, plus a code fileset at
/// `-C 500`): at the original 1e9 arrays never rotated — only exact-tie
/// object keys did; 1e13 moved the frontier ±1 index per run; 1e14 reached
/// ±2 while mostly re-showing anchors; 1e15 produced near-disjoint fresh
/// rings per run while loose budgets (`-c 100000`) still rendered every item
/// (the penalty reorders, never excludes). Larger scales push the no-go
/// radius past what tight budgets can render, wasting budget on the
/// structural shells of suppressed slots, so 1e15 is the chosen balance.
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
        for (node_id, delta) in &penalties {
            bump_score(order, *node_id, *delta);
        }
        order.by_priority.sort_by_key(|id| order.scores[id.0]);
    }
    order.merkle_hashes = Some(hashes);
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
}
