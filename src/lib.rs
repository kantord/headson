#![doc = include_str!("../README.md")]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr
)]
#![allow(
    clippy::multiple_crate_versions,
    reason = "Dependency graph pulls distinct versions (e.g., yaml-rust2)."
)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "tests may use unwrap/expect for brevity"
    )
)]

use anyhow::Result;

pub mod budget;
mod debug;
mod grep;
mod ingest;
pub mod node_path;
mod order;
mod pruner;
mod serialization;
mod utils;
pub use grep::{
    GrepConfig, GrepPatterns, GrepShow, build_grep_config,
    build_grep_config_from_patterns, combine_patterns,
};
pub use ingest::fileset::{FilesetInput, FilesetInputKind};
pub use ingest::format::Format;
pub use ingest::{parse_json_one, parse_text_one_with_mode};
pub use order::types::{ArrayBias, ArraySamplerStrategy};
pub use order::types::{Breadcrumb, ExploreContext};
pub use order::{
    DEFAULT_SAFETY_CAP, NodeId, NodeKind, PriorityConfig, PriorityOrder,
    RankedNode, build_order,
};
pub use utils::extensions;
pub use utils::templates::map_json_template_for_style;

pub use node_path::{compute_merkle_hashes, leaf_breadcrumb_key};
pub use pruner::budget::find_largest_render_under_budgets;
pub use prunist::{Budget, BudgetKind, Budgets};
pub use serialization::color::resolve_color_enabled;
pub use serialization::types::{
    ColorMode, ColorStrategy, OutputTemplate, RenderConfig, Style,
};

#[derive(Debug, Clone, Copy)]
pub struct MatchSummary {
    pub shown: usize,
    pub hidden: usize,
}

#[derive(Debug)]
pub struct RenderOutput {
    pub text: String,
    pub warnings: Vec<String>,
    pub match_summary: Option<MatchSummary>,
    /// `(file, path)` pairs for leaf nodes in the rendered output.
    /// Used by CLI session middleware to record breadcrumbs. `file` is `""`
    /// for single-file inputs; `path` is a dot-joined key chain or content hash.
    pub shown_leaves: Vec<(String, String)>,
}

#[derive(Copy, Clone, Debug)]
pub enum TextMode {
    Plain,
    CodeLike,
}

pub enum InputKind {
    Json(Vec<u8>),
    Jsonl(Vec<u8>),
    Yaml(Vec<u8>),
    Text { bytes: Vec<u8>, mode: TextMode },
    Fileset(Vec<FilesetInput>),
}

fn apply_explore_context(order: &mut PriorityOrder, ctx: &ExploreContext) {
    let hashes = node_path::compute_merkle_hashes(order);
    let penalties: Vec<(NodeId, u128)> = order
        .by_priority
        .iter()
        .filter_map(|&node_id| {
            let (file, path) =
                node_path::leaf_breadcrumb_key(order, node_id, &hashes)?;
            let bc = ctx
                .breadcrumbs
                .iter()
                .find(|b| b.file == file && b.path == path)?;
            let steps_ago = ctx.current_step.saturating_sub(bc.last_step);
            let penalty = (1.0 + bc.count as f64).ln()
                * ctx.alpha.powi(steps_ago as i32);
            (penalty > 0.0)
                .then_some((node_id, (penalty * 1_000_000_000.0) as u128))
        })
        .collect();
    for (node_id, delta) in &penalties {
        order.scores[node_id.0] =
            order.scores[node_id.0].saturating_add(*delta);
    }
    order.by_priority.sort_by_key(|id| order.scores[id.0]);
}

pub fn headson(
    input: InputKind,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    grep: &GrepConfig,
    budgets: Budgets,
) -> Result<RenderOutput> {
    let crate::ingest::IngestOutput {
        arena,
        mut warnings,
    } = crate::ingest::ingest_into_arena(input, priority_cfg, grep)?;
    let mut order_build = order::build_order(&arena, priority_cfg)?;
    if let Some(ctx) = &priority_cfg.explore {
        apply_explore_context(&mut order_build, ctx);
    }
    if order_build.safety_cap_hit {
        warnings.push(format!(
            "warning: input truncated (exceeded {} node safety cap)",
            priority_cfg.safety_cap
        ));
    }
    let (text, match_summary, top_k) = find_largest_render_under_budgets(
        &mut order_build,
        config,
        grep,
        budgets,
    );
    let shown_leaves = node_path::collect_shown_leaves(&order_build, top_k);
    Ok(RenderOutput {
        text,
        warnings,
        match_summary,
        shown_leaves,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_render_config() -> RenderConfig {
        RenderConfig {
            template: OutputTemplate::Pseudo,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            color_mode: ColorMode::Off,
            color_enabled: false,
            style: serialization::types::Style::Default,
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

    #[test]
    fn safety_cap_warning_emitted_when_exceeded() {
        // Use a tiny safety cap so we can trigger it with minimal input.
        // An array [1,2,3,4,5] generates: 1 root array + 5 children = 6 nodes.
        // With safety_cap=5, we should hit the cap.
        let mut priority_cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        priority_cfg.safety_cap = 5;

        let result = headson(
            InputKind::Json(b"[1,2,3,4,5]".to_vec()),
            &test_render_config(),
            &priority_cfg,
            &GrepConfig::default(),
            Budgets::default(),
        )
        .expect("headson should succeed");

        assert!(
            result.warnings.iter().any(|w| w.contains("safety cap")),
            "expected safety cap warning, got: {:?}",
            result.warnings
        );
    }

    #[test]
    fn no_safety_cap_warning_when_not_exceeded() {
        // With default (2M) cap, a small input should not trigger warning.
        let priority_cfg = PriorityConfig::new(usize::MAX, usize::MAX);

        let result = headson(
            InputKind::Json(b"[1,2,3]".to_vec()),
            &test_render_config(),
            &priority_cfg,
            &GrepConfig::default(),
            Budgets::default(),
        )
        .expect("headson should succeed");

        assert!(
            !result.warnings.iter().any(|w| w.contains("safety cap")),
            "unexpected safety cap warning: {:?}",
            result.warnings
        );
    }

    #[test]
    fn strong_grep_match_summary_hidden_zero_under_tight_budget() {
        // JSON object with 3 keys: two contain "needle", one does not.
        // A 1-line global budget would normally suppress most output, but strong
        // grep must override it and include all matching nodes.
        let input = br#"{"alpha": "needle one", "beta": "no match here", "gamma": "needle two"}"#;
        let grep_cfg = build_grep_config(
            Some("needle"),
            None,
            GrepShow::Matching,
            false,
            true,
        )
        .expect("valid grep pattern");
        let priority_cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        // Tight global budget: 1 line — far too small to render all 4 nodes
        // (root object + 3 values) without grep forcing matches in.
        let budgets = Budgets {
            global: Some(Budget {
                kind: BudgetKind::Lines,
                cap: 1,
            }),
            per_slot: None,
        };

        let result = headson(
            InputKind::Json(input.to_vec()),
            &test_render_config(),
            &priority_cfg,
            &grep_cfg,
            budgets,
        )
        .expect("headson should succeed");

        let summary = result
            .match_summary
            .expect("match_summary must be Some when grep is active");
        assert_eq!(
            summary.hidden, 0,
            "strong grep must force all matches into output; hidden should be 0, got {:?}",
            result.match_summary
        );
        assert_eq!(
            summary.shown, 2,
            "exactly 2 values match 'needle'; shown should be 2, got {:?}",
            result.match_summary
        );
    }

    #[test]
    fn strong_grep_match_summary_zero_matches() {
        let input = br#"{"alpha": "apple", "beta": "banana"}"#;
        let grep_cfg = build_grep_config(
            Some("zzznomatch"),
            None,
            GrepShow::Matching,
            false,
            true,
        )
        .expect("valid grep pattern");
        let priority_cfg = PriorityConfig::new(usize::MAX, usize::MAX);

        let result = headson(
            InputKind::Json(input.to_vec()),
            &test_render_config(),
            &priority_cfg,
            &grep_cfg,
            Budgets::default(),
        )
        .expect("headson should succeed");

        let summary = result
            .match_summary
            .expect("match_summary must be Some when grep is active");
        assert_eq!(
            summary.shown, 0,
            "pattern matches nothing; shown should be 0, got {:?}",
            result.match_summary
        );
        assert_eq!(
            summary.hidden, 0,
            "pattern matches nothing; hidden should be 0, got {:?}",
            result.match_summary
        );
    }

    #[test]
    fn weak_grep_match_summary_has_hidden_under_tight_budget() {
        // JSON object with 5 keys, all values contain "target".
        // Pseudo rendering (one key-value per line plus { and }) needs 7 lines
        // for the full object. A 4-line global budget fits only a subset.
        let input = br#"{
            "a": "target one",
            "b": "target two",
            "c": "target three",
            "d": "target four",
            "e": "target five"
        }"#;
        let total_matches: usize = 5;
        let grep_cfg = build_grep_config(
            None,
            Some("target"),
            GrepShow::Matching,
            false,
            false,
        )
        .expect("valid weak grep pattern");
        let priority_cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        // 4-line budget: enough to show the object braces plus ~2 values,
        // not enough to show all 5 matches.
        let budgets = Budgets {
            global: Some(Budget {
                kind: BudgetKind::Lines,
                cap: 4,
            }),
            per_slot: None,
        };

        let result = headson(
            InputKind::Json(input.to_vec()),
            &test_render_config(),
            &priority_cfg,
            &grep_cfg,
            budgets,
        )
        .expect("headson should succeed");

        let summary = result
            .match_summary
            .expect("match_summary must be Some when grep is active");
        assert_eq!(
            summary.shown + summary.hidden,
            total_matches,
            "shown + hidden must equal total direct matches ({}); got shown={} hidden={}",
            total_matches,
            summary.shown,
            summary.hidden,
        );
        assert!(
            summary.hidden > 0,
            "tight budget must cause some weak-grep matches to be hidden; \
             got shown={} hidden={}",
            summary.shown,
            summary.hidden,
        );
    }

    #[test]
    fn weak_grep_match_summary_all_shown_under_loose_budget() {
        // Same 5-value object as the tight-budget test; with default (no) budget
        // all matches should appear in the output.
        let input = br#"{
            "a": "target one",
            "b": "target two",
            "c": "target three",
            "d": "target four",
            "e": "target five"
        }"#;
        let total_matches: usize = 5;
        let grep_cfg = build_grep_config(
            None,
            Some("target"),
            GrepShow::Matching,
            false,
            false,
        )
        .expect("valid weak grep pattern");
        let priority_cfg = PriorityConfig::new(usize::MAX, usize::MAX);

        let result = headson(
            InputKind::Json(input.to_vec()),
            &test_render_config(),
            &priority_cfg,
            &grep_cfg,
            Budgets::default(),
        )
        .expect("headson should succeed");

        let summary = result
            .match_summary
            .expect("match_summary must be Some when grep is active");
        assert_eq!(
            summary.shown, total_matches,
            "loose budget must show all {} matches; got shown={} hidden={}",
            total_matches, summary.shown, summary.hidden,
        );
        assert_eq!(
            summary.hidden, 0,
            "no matches should be hidden under a loose budget; \
             got shown={} hidden={}",
            summary.shown, summary.hidden,
        );
    }

    // ── Step 25: find_largest_render_under_budgets returns a meaningful top_k ─

    /// Build a `PriorityOrder` from a small JSON object for use in top_k tests.
    fn make_order_for_top_k() -> PriorityOrder {
        let input = InputKind::Json(
            br#"{"a": 1, "b": 2, "c": 3, "d": 4, "e": 5}"#.to_vec(),
        );
        let priority_cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let grep_cfg = GrepConfig::default();
        let ingest_out =
            crate::ingest::ingest_into_arena(input, &priority_cfg, &grep_cfg)
                .expect("ingest must succeed");
        order::build_order(&ingest_out.arena, &priority_cfg)
            .expect("build_order must succeed")
    }

    #[test]
    fn find_largest_render_returns_nonzero_top_k_under_tight_budget() {
        // A 15-byte budget is far too small to render all 5 key-value pairs of
        // {"a":1,"b":2,"c":3,"d":4,"e":5} (full render ≈ 44 bytes), so the
        // budget search must stop before including all nodes.
        let mut order = make_order_for_top_k();
        let total_nodes = order.total_nodes;
        let tight_budgets = Budgets {
            global: Some(Budget {
                kind: BudgetKind::Bytes,
                cap: 15,
            }),
            per_slot: None,
        };
        let grep_cfg = GrepConfig::default();

        let (_text, _summary, top_k) = find_largest_render_under_budgets(
            &mut order,
            &test_render_config(),
            &grep_cfg,
            tight_budgets,
        );

        assert!(
            top_k > 0,
            "top_k must be > 0 (at least one node was selected); got {top_k}"
        );
        assert!(
            top_k < total_nodes,
            "tight budget must not select all {total_nodes} nodes; top_k={top_k}"
        );
    }

    // ── ExploreContext integration: deprioritize seen nodes via PriorityConfig ─

    /// Compute the composite breadcrumb key (`"dot_path#hash"`) for a leaf at
    /// `dot_path` in a single-file JSON input.
    fn composite_key_for(json: &[u8], dot_path: &str) -> String {
        let input = InputKind::Json(json.to_vec());
        let prio = PriorityConfig::new(usize::MAX, usize::MAX);
        let grep = GrepConfig::default();
        let ingest_out = crate::ingest::ingest_into_arena(input, &prio, &grep)
            .expect("ingest");
        let order =
            order::build_order(&ingest_out.arena, &prio).expect("order");
        let hashes = node_path::compute_merkle_hashes(&order);
        order
            .by_priority
            .iter()
            .find_map(|&node_id| {
                let (_, path) =
                    node_path::leaf_breadcrumb_key(&order, node_id, &hashes)?;
                let prefix = path.split_once('#').map(|(p, _)| p)?;
                (prefix == dot_path).then_some(path)
            })
            .unwrap_or_else(|| panic!("no leaf at dot_path {dot_path:?}"))
    }

    /// When `headson()` is called with an `ExploreContext` that records a
    /// breadcrumb for the 'a' key in `{"a": 1, "b": 2}`, under a tight global
    /// byte budget (20 bytes), the rendered output must contain 'b' but not 'a'.
    #[test]
    fn explore_context_deprioritizes_seen_node_under_tight_budget() {
        let crumb = Breadcrumb {
            file: "".to_string(),
            path: composite_key_for(br#"{"a": 1, "b": 2}"#, "a"),
            count: 1,
            last_step: 1,
        };
        let ctx = ExploreContext {
            breadcrumbs: vec![crumb],
            current_step: 2,
            alpha: 0.5,
        };
        let mut prio = PriorityConfig::new(usize::MAX, usize::MAX);
        prio.explore = Some(ctx);

        let tight_budgets = Budgets {
            global: Some(Budget {
                kind: BudgetKind::Bytes,
                cap: 20,
            }),
            per_slot: None,
        };

        let result = headson(
            InputKind::Json(br#"{"a": 1, "b": 2}"#.to_vec()),
            &test_render_config(),
            &prio,
            &GrepConfig::default(),
            tight_budgets,
        )
        .expect("headson should succeed");

        assert!(
            !result.text.contains("a:")
                && !result.text.contains("a :")
                && !result.text.contains("\"a\""),
            "penalized key 'a' must NOT appear in output under tight budget; got: {:?}",
            result.text
        );
        assert!(
            result.text.contains('b'),
            "un-penalized key 'b' must appear in output; got: {:?}",
            result.text
        );
    }

    /// Baseline: with `explore = None` and the same tight budget, 'a' appears
    /// (it has higher priority than 'b' under fair object-key ordering).
    #[test]
    fn explore_context_none_does_not_affect_output() {
        let mut prio = PriorityConfig::new(usize::MAX, usize::MAX);
        prio.explore = None;

        let tight_budgets = Budgets {
            global: Some(Budget {
                kind: BudgetKind::Bytes,
                cap: 20,
            }),
            per_slot: None,
        };

        let result = headson(
            InputKind::Json(br#"{"a": 1, "b": 2}"#.to_vec()),
            &test_render_config(),
            &prio,
            &GrepConfig::default(),
            tight_budgets,
        )
        .expect("headson should succeed");

        assert!(
            result.text.contains('a'),
            "without explore penalty, 'a' (first key) must appear in output under tight budget; got: {:?}",
            result.text
        );
    }

    // ── Step 26: top_k slice contains at least one leaf node ──────────────────

    #[test]
    fn top_k_slice_contains_at_least_one_leaf_node() {
        // The top_k slice from by_priority should include real leaf nodes
        // (AtomicLeaf or SplittableLeaf) — not just ancestor scaffolding.
        // 30 bytes comfortably fits root + 1-2 leaves (~16 bytes each) but
        // not all 5 leaves (~44 bytes total), so top_k is neither 0 nor max.
        let mut order = make_order_for_top_k();
        let tight_budgets = Budgets {
            global: Some(Budget {
                kind: BudgetKind::Bytes,
                cap: 30,
            }),
            per_slot: None,
        };
        let grep_cfg = GrepConfig::default();

        let (_text, _summary, top_k) = find_largest_render_under_budgets(
            &mut order,
            &test_render_config(),
            &grep_cfg,
            tight_budgets,
        );

        let has_leaf = order.by_priority[..top_k].iter().any(|node_id| {
            matches!(
                order.nodes[node_id.0],
                RankedNode::AtomicLeaf { .. }
                    | RankedNode::SplittableLeaf { .. }
            )
        });

        assert!(
            has_leaf,
            "by_priority[..{}] must contain at least one AtomicLeaf or \
             SplittableLeaf; nodes in slice: {:?}",
            top_k,
            order.by_priority[..top_k]
                .iter()
                .map(|id| &order.nodes[id.0])
                .collect::<Vec<_>>(),
        );
    }
}
