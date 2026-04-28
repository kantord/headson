use headson::{NodeId, PriorityOrder};

use crate::session::Session;

pub fn apply_explore_penalty(
    order: &mut PriorityOrder,
    session: &Session,
    current_step: u64,
    alpha: f64,
) {
    let penalties: Vec<(NodeId, u128)> = order
        .by_priority
        .iter()
        .filter_map(|&node_id| {
            let (file, path) = crate::session::breadcrumb_key(order, node_id)?;
            let p = session.penalty_for(&file, &path, current_step, alpha);
            (p > 0.0).then_some((node_id, (p * 1_000_000_000.0) as u128))
        })
        .collect();
    for (node_id, delta) in &penalties {
        order.scores[node_id.0] =
            order.scores[node_id.0].saturating_add(*delta);
    }
    order.by_priority.sort_by_key(|id| order.scores[id.0]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use headson::{
        Budget, BudgetKind, Budgets, ColorMode, GrepConfig, OutputTemplate,
        PriorityConfig, RankedNode, RenderConfig, Style, build_order,
        find_largest_render_under_budgets, parse_json_one,
    };

    use crate::session::breadcrumb_key;

    fn test_render_config() -> RenderConfig {
        RenderConfig {
            template: OutputTemplate::Pseudo,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            color_mode: ColorMode::Off,
            color_enabled: false,
            style: Style::Default,
            prefer_tail_arrays: false,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: false,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        }
    }

    fn build_json_order(json: &[u8]) -> PriorityOrder {
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let arena = parse_json_one(json.to_vec(), &cfg).expect("parse");
        build_order(&arena, &cfg).expect("order")
    }

    fn find_leaf_for_token(
        order: &PriorityOrder,
        token: &str,
    ) -> Option<NodeId> {
        order.nodes.iter().find_map(|n| match n {
            RankedNode::AtomicLeaf {
                node_id, token: t, ..
            } if t == token => Some(*node_id),
            RankedNode::SplittableLeaf {
                node_id, value: v, ..
            } if v == token => Some(*node_id),
            _ => None,
        })
    }

    /// Step 27 — Unseen nodes: applying explore penalty leaves by_priority unchanged.
    ///
    /// When no breadcrumbs have been recorded (fresh session), `apply_explore_penalty`
    /// should be a no-op: the priority ordering must remain identical to the snapshot
    /// taken before the call.
    #[test]
    fn step_27_unseen_nodes_penalty_leaves_order_unchanged() {
        let mut order = build_json_order(br#"{"a": 1, "b": 2}"#);
        let before: Vec<NodeId> = order.by_priority.clone();

        let session = Session::new("id".to_string(), "lbl".to_string());
        apply_explore_penalty(&mut order, &session, 1, 0.5);

        assert_eq!(
            order.by_priority, before,
            "by_priority must be unchanged when no breadcrumbs have been recorded"
        );
    }

    /// Step 28 — Seen node moves toward the back of by_priority.
    ///
    /// After recording a breadcrumb for the leaf whose path resolves to "a",
    /// `apply_explore_penalty` must push that leaf toward the back of `by_priority`
    /// (higher score = lower priority = later position) so that the "b" leaf
    /// appears before the "a" leaf.
    #[test]
    fn step_28_seen_node_moves_toward_back_of_by_priority() {
        let mut order = build_json_order(br#"{"a": 1, "b": 2}"#);

        // Find the leaf NodeId for value "1" (the value of key "a")
        let leaf_a_id = find_leaf_for_token(&order, "1")
            .expect("should find leaf for value 1 (key 'a')");

        // Verify it resolves to path "a"
        let (file, path) = breadcrumb_key(&order, leaf_a_id)
            .expect("leaf_a should return a breadcrumb key");
        assert_eq!(path, "a", "leaf for value 1 should have path 'a'");

        // Find the leaf NodeId for value "2" (the value of key "b")
        let leaf_b = find_leaf_for_token(&order, "2")
            .expect("should find leaf for value 2 (key 'b')");

        // Record a breadcrumb for the "a" leaf at step 1
        let mut session = Session::new("id".to_string(), "lbl".to_string());
        session.record_breadcrumb(&file, &path, 1);

        // Apply penalty at current_step=2
        apply_explore_penalty(&mut order, &session, 2, 0.5);

        // Both leaf_a_id and leaf_b should appear in by_priority
        let pos_a = order
            .by_priority
            .iter()
            .position(|&id| id == leaf_a_id)
            .expect("leaf_a must remain in by_priority after penalty");
        let pos_b = order
            .by_priority
            .iter()
            .position(|&id| id == leaf_b)
            .expect("leaf_b must remain in by_priority after penalty");

        assert!(
            pos_b < pos_a,
            "after penalty, leaf_b (unseen) should appear BEFORE leaf_a (seen); \
             pos_b={pos_b}, pos_a={pos_a}"
        );
    }

    /// Step 29 — Penalty is soft: penalized node still appears under a loose budget.
    ///
    /// Under a generous (default) budget both "a" and "b" leaves fit in the output.
    /// Applying a penalty for "a" must NOT cause "a" to disappear — it is only
    /// deprioritised, not excluded.
    #[test]
    fn step_29_penalized_node_still_appears_under_loose_budget() {
        let input = br#"{"a": 1, "b": 2}"#;
        let priority_cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let config = test_render_config();
        let grep_cfg = GrepConfig::default();

        // Run 1 (baseline): no penalty applied.
        let arena1 =
            parse_json_one(input.to_vec(), &priority_cfg).expect("parse");
        let mut order1 = build_order(&arena1, &priority_cfg).expect("order");
        let (text1, _, _) = find_largest_render_under_budgets(
            &mut order1,
            &config,
            &grep_cfg,
            Budgets::default(),
        );
        assert!(
            text1.contains('a'),
            "baseline: 'a' must appear in output; got: {text1:?}"
        );

        // Run 2 (penalized): record breadcrumb for leaf at path "a", apply penalty.
        let arena2 =
            parse_json_one(input.to_vec(), &priority_cfg).expect("parse");
        let mut order2 = build_order(&arena2, &priority_cfg).expect("order");

        // Find the leaf for key "a" (value token "1") and confirm its path.
        let leaf_a_id = order2
            .nodes
            .iter()
            .find_map(|n| match n {
                RankedNode::AtomicLeaf { node_id, token, .. }
                    if token == "1" =>
                {
                    Some(*node_id)
                }
                _ => None,
            })
            .expect(
                "should find AtomicLeaf with token '1' (value of key 'a')",
            );
        let (file, path) = breadcrumb_key(&order2, leaf_a_id)
            .expect("leaf_a should produce a breadcrumb key");
        assert_eq!(path, "a", "leaf for value 1 should have path 'a'");

        let mut session = Session::new("id".to_string(), "lbl".to_string());
        session.record_breadcrumb(&file, &path, 1);
        apply_explore_penalty(&mut order2, &session, 2, 0.5);

        let (text2, _, _) = find_largest_render_under_budgets(
            &mut order2,
            &config,
            &grep_cfg,
            Budgets::default(),
        );
        assert!(
            text2.contains('a'),
            "penalized but loose budget: 'a' must still appear in output; got: {text2:?}"
        );
    }

    /// Step 30 — Penalty is effective: penalized node is absent under a tight budget.
    ///
    /// With a ~20-byte global budget only one leaf's key-value fits in the output
    /// (the object structure + one leaf consumes the budget).  After penalising
    /// "a", the renderer must pick "b" (the unseen leaf) instead.
    ///
    /// Budget probe: `{"a": 1, "b": 2}` with 20 global bytes renders exactly one
    /// leaf (verified empirically: 18-24 bytes shows one leaf + omission marker).
    #[test]
    fn step_30_penalized_node_absent_under_tight_budget() {
        let input = br#"{"a": 1, "b": 2}"#;
        let priority_cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let config = test_render_config();
        let grep_cfg = GrepConfig::default();

        fn tight() -> Budgets {
            Budgets {
                global: Some(Budget {
                    kind: BudgetKind::Bytes,
                    cap: 20,
                }),
                per_slot: None,
            }
        }

        // Run 1 (baseline, no penalty): establish that the tight budget renders
        // exactly one of the two leaves (we don't care which one here).
        let arena1 =
            parse_json_one(input.to_vec(), &priority_cfg).expect("parse");
        let mut order1 = build_order(&arena1, &priority_cfg).expect("order");
        let (text1, _, _) = find_largest_render_under_budgets(
            &mut order1,
            &config,
            &grep_cfg,
            tight(),
        );
        // At least one leaf must appear — confirms the budget isn't so tight nothing renders.
        assert!(
            text1.contains('a') || text1.contains('b'),
            "baseline tight: at least one leaf must appear; got: {text1:?}"
        );
        // Both leaves must NOT both appear — confirms the budget is actually tight.
        assert!(
            !(text1.contains('a') && text1.contains('b')),
            "baseline tight: both leaves must NOT fit; budget should suppress one; got: {text1:?}"
        );

        // Run 2 (with penalty on "a"): "a" must be pushed out, "b" must appear.
        let arena2 =
            parse_json_one(input.to_vec(), &priority_cfg).expect("parse");
        let mut order2 = build_order(&arena2, &priority_cfg).expect("order");

        let leaf_a_id = order2
            .nodes
            .iter()
            .find_map(|n| match n {
                RankedNode::AtomicLeaf { node_id, token, .. }
                    if token == "1" =>
                {
                    Some(*node_id)
                }
                _ => None,
            })
            .expect(
                "should find AtomicLeaf with token '1' (value of key 'a')",
            );
        let (file, path) = breadcrumb_key(&order2, leaf_a_id)
            .expect("leaf_a should produce a breadcrumb key");
        assert_eq!(path, "a");

        let mut session = Session::new("id".to_string(), "lbl".to_string());
        session.record_breadcrumb(&file, &path, 1);
        apply_explore_penalty(&mut order2, &session, 2, 0.5);

        let (text2, _, _) = find_largest_render_under_budgets(
            &mut order2,
            &config,
            &grep_cfg,
            tight(),
        );

        // The "a" key-value pair must NOT appear.
        assert!(
            !text2.contains("a:") && !text2.contains("a :"),
            "penalized tight: 'a' key must NOT appear in output; got: {text2:?}"
        );
        // The "b" key-value pair MUST appear.
        assert!(
            text2.contains('b'),
            "penalized tight: 'b' must appear in output; got: {text2:?}"
        );
    }
}
