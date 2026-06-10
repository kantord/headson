use std::collections::HashMap;

use crate::node_path;
use crate::order::types::{
    Breadcrumb, ExploreContext, NodeId, PriorityOrder, RankedNode,
    novelty_penalty,
};

// Converts f64 penalty to u128 score units; large enough to preserve ordering resolution.
const PENALTY_SCALE: f64 = 1_000_000_000.0;

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
    let by_key: HashMap<(&str, &str), &Breadcrumb> = ctx
        .breadcrumbs
        .iter()
        .map(|b| ((b.file.as_str(), b.path.as_str()), b))
        .collect();
    let penalties: Vec<(NodeId, u128)> = order
        .by_priority
        .iter()
        .filter_map(|&node_id| {
            let (file, path) =
                node_path::leaf_breadcrumb_key(order, node_id, &hashes)?;
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
        order
            .by_priority
            .iter()
            .find_map(|&node_id| {
                let key =
                    node_path::leaf_breadcrumb_key(order, node_id, &hashes)?;
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
}
