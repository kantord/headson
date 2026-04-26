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
pub use order::types::{ArrayBias, ArraySamplerStrategy};
pub use order::{
    DEFAULT_SAFETY_CAP, NodeId, NodeKind, PriorityConfig, PriorityOrder,
    RankedNode, build_order,
};
pub use utils::extensions;
pub use utils::templates::map_json_template_for_style;

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
    if order_build.safety_cap_hit {
        warnings.push(format!(
            "warning: input truncated (exceeded {} node safety cap)",
            priority_cfg.safety_cap
        ));
    }
    let (text, match_summary) = find_largest_render_under_budgets(
        &mut order_build,
        config,
        grep,
        budgets,
    );
    Ok(RenderOutput {
        text,
        warnings,
        match_summary,
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
        let grep_cfg =
            build_grep_config(Some("needle"), None, GrepShow::Matching, false)
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
        let grep_cfg =
            build_grep_config(None, Some("target"), GrepShow::Matching, false)
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
        let grep_cfg =
            build_grep_config(None, Some("target"), GrepShow::Matching, false)
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
}
