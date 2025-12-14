use super::*;
use crate::order::types::NodeMetrics;
use crate::order::{
    NodeId, ObjectType, PriorityOrder, RankedNode, build_order,
};
use insta::assert_snapshot;
use std::collections::HashMap;

fn assert_yaml_valid(s: &str) {
    let _: serde_yaml::Value =
        serde_yaml::from_str(s).expect("YAML parse failed (validation)");
}

fn unbounded_prio() -> crate::PriorityConfig {
    crate::PriorityConfig::new(usize::MAX, usize::MAX)
}

fn render_cfg(
    template: crate::OutputTemplate,
    style: crate::serialization::types::Style,
) -> crate::RenderConfig {
    crate::RenderConfig {
        template,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: crate::ColorMode::Off,
        color_enabled: false,
        style,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        fileset_tree: false,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    }
}

#[test]
fn arena_render_empty_array() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[]",
        &unbounded_prio(),
    )
    .unwrap();
    let build = build_order(&arena, &unbounded_prio()).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let cfg = crate::RenderConfig {
        color_mode: crate::ColorMode::Auto,
        ..render_cfg(
            crate::OutputTemplate::Json,
            crate::serialization::types::Style::Strict,
        )
    };
    let out = render_top_k(&build, 10, &mut marks, 1, &cfg);
    assert_snapshot!("arena_render_empty", out);
}

#[test]
fn newline_detection_crlf_array_child() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[{\"a\":1,\"b\":2}]",
        &unbounded_prio(),
    )
    .unwrap();
    let build = build_order(&arena, &unbounded_prio()).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let cfg = crate::RenderConfig {
        newline: "\r\n".to_string(),
        color_mode: crate::ColorMode::Auto,
        ..render_cfg(
            crate::OutputTemplate::Json,
            crate::serialization::types::Style::Strict,
        )
    };
    let out = render_top_k(&build, usize::MAX, &mut marks, 1, &cfg);
    assert!(
        out.contains("\r\n"),
        "expected CRLF newlines in output: {out:?}"
    );
    assert!(out.starts_with("["));
}

#[test]
fn arena_render_single_string_array() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[\"ab\"]",
        &unbounded_prio(),
    )
    .unwrap();
    let build = build_order(&arena, &unbounded_prio()).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let cfg = crate::RenderConfig {
        color_mode: crate::ColorMode::Auto,
        ..render_cfg(
            crate::OutputTemplate::Json,
            crate::serialization::types::Style::Strict,
        )
    };
    let out = render_top_k(&build, 10, &mut marks, 1, &cfg);
    assert_snapshot!("arena_render_single", out);
}

#[test]
fn array_omitted_markers_pseudo_head_and_tail() {
    let cfg_prio = crate::PriorityConfig {
        max_string_graphemes: usize::MAX,
        array_max_items: 1,
        prefer_tail_arrays: false,
        array_bias: crate::ArrayBias::HeadMidTail,
        array_sampler: crate::ArraySamplerStrategy::Default,
        line_budget_only: false,
    };
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[1,2,3]", &cfg_prio,
    )
    .unwrap();
    let build = build_order(&arena, &cfg_prio).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let base_cfg = render_cfg(
        crate::OutputTemplate::Pseudo,
        crate::serialization::types::Style::Default,
    );

    let out_head = render_top_k(&build, 2, &mut marks, 1, &base_cfg);
    assert_snapshot!("array_omitted_pseudo_head", out_head);

    let out_tail = render_top_k(
        &build,
        2,
        &mut marks,
        2,
        &crate::RenderConfig {
            prefer_tail_arrays: true,
            ..base_cfg.clone()
        },
    );
    assert_snapshot!("array_omitted_pseudo_tail", out_tail);
}

#[test]
fn array_omitted_markers_js_head_and_tail() {
    let cfg_prio = crate::PriorityConfig {
        max_string_graphemes: usize::MAX,
        array_max_items: 1,
        prefer_tail_arrays: false,
        array_bias: crate::ArrayBias::HeadMidTail,
        array_sampler: crate::ArraySamplerStrategy::Default,
        line_budget_only: false,
    };
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[1,2,3]", &cfg_prio,
    )
    .unwrap();
    let build = build_order(&arena, &cfg_prio).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let base_cfg = render_cfg(
        crate::OutputTemplate::Js,
        crate::serialization::types::Style::Detailed,
    );

    let out_head = render_top_k(&build, 2, &mut marks, 3, &base_cfg);
    assert_snapshot!("array_omitted_js_head", out_head);

    let out_tail = render_top_k(
        &build,
        2,
        &mut marks,
        4,
        &crate::RenderConfig {
            prefer_tail_arrays: true,
            ..base_cfg.clone()
        },
    );
    assert_snapshot!("array_omitted_js_tail", out_tail);
}

#[test]
fn array_omitted_markers_yaml_head_and_tail() {
    let cfg_prio = crate::PriorityConfig {
        max_string_graphemes: usize::MAX,
        array_max_items: 1,
        prefer_tail_arrays: false,
        array_bias: crate::ArrayBias::HeadMidTail,
        array_sampler: crate::ArraySamplerStrategy::Default,
        line_budget_only: false,
    };
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[1,2,3]", &cfg_prio,
    )
    .unwrap();
    let build = build_order(&arena, &cfg_prio).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let base_cfg = render_cfg(
        crate::OutputTemplate::Yaml,
        crate::serialization::types::Style::Detailed,
    );

    let out_head = render_top_k(&build, 2, &mut marks, 11, &base_cfg);
    assert_yaml_valid(&out_head);
    assert_snapshot!("array_omitted_yaml_head", out_head);

    let out_tail = render_top_k(
        &build,
        2,
        &mut marks,
        12,
        &crate::RenderConfig {
            prefer_tail_arrays: true,
            ..base_cfg.clone()
        },
    );
    assert_yaml_valid(&out_tail);
    assert_snapshot!("array_omitted_yaml_tail", out_tail);
}

#[test]
fn arena_render_empty_array_yaml() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[]",
        &unbounded_prio(),
    )
    .unwrap();
    let build = build_order(&arena, &unbounded_prio()).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let cfg = crate::RenderConfig {
        color_mode: crate::ColorMode::Auto,
        ..render_cfg(
            crate::OutputTemplate::Yaml,
            crate::serialization::types::Style::Default,
        )
    };
    let out = render_top_k(&build, 10, &mut marks, 21, &cfg);
    assert_yaml_valid(&out);
    assert_snapshot!("arena_render_empty_yaml", out);
}

#[test]
fn arena_render_single_string_array_yaml() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[\"ab\"]",
        &unbounded_prio(),
    )
    .unwrap();
    let build = build_order(&arena, &unbounded_prio()).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let cfg = crate::RenderConfig {
        color_mode: crate::ColorMode::Auto,
        ..render_cfg(
            crate::OutputTemplate::Yaml,
            crate::serialization::types::Style::Default,
        )
    };
    let out = render_top_k(&build, 10, &mut marks, 22, &cfg);
    assert_yaml_valid(&out);
    assert_snapshot!("arena_render_single_yaml", out);
}

#[test]
fn inline_open_array_in_object_yaml() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "{\"a\":[1,2,3]}",
        &crate::PriorityConfig::new(usize::MAX, 2),
    )
    .unwrap();
    let build =
        build_order(&arena, &crate::PriorityConfig::new(usize::MAX, 2))
            .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let cfg = render_cfg(
        crate::OutputTemplate::Yaml,
        crate::serialization::types::Style::Detailed,
    );
    let out = render_top_k(&build, 4, &mut marks, 23, &cfg);
    assert_yaml_valid(&out);
    assert_snapshot!("inline_open_array_in_object_yaml", out);
}

fn mk_gap_ctx() -> super::templates::ArrayCtx<'static> {
    super::templates::ArrayCtx {
        children: vec![
            (0, (crate::order::NodeKind::Number, "1".to_string())),
            (3, (crate::order::NodeKind::Number, "2".to_string())),
            (5, (crate::order::NodeKind::Number, "3".to_string())),
        ],
        children_len: 3,
        omitted: 0,
        depth: 0,
        inline_open: false,
        omitted_at_start: false,
        source_hint: None,
        code_highlight: None,
    }
}

fn assert_contains_all(out: &str, needles: &[&str]) {
    needles.iter().for_each(|n| assert!(out.contains(n)));
}

#[test]
fn array_internal_gaps_yaml() {
    let ctx = mk_gap_ctx();
    let mut s = String::new();
    let cfg = render_cfg(
        crate::OutputTemplate::Yaml,
        crate::serialization::types::Style::Default,
    );
    let mut outw = crate::serialization::output::Out::new(&mut s, &cfg, None);
    super::templates::render_array(
        crate::OutputTemplate::Yaml,
        &ctx,
        &mut outw,
    );
    let out = s;
    assert_yaml_valid(&out);
    assert_snapshot!("array_internal_gaps_yaml", out);
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "Aggregated YAML quoting cases in one test to reuse setup."
)]
fn yaml_key_and_scalar_quoting() {
    let json = "{\n            \"true\": 1,\n            \"010\": \"010\",\n            \"-dash\": \"ok\",\n            \"normal\": \"simple\",\n            \"a:b\": \"a:b\",\n            \" spaced \": \" spaced \",\n            \"reserved\": \"yes\",\n            \"multiline\": \"line1\\nline2\"\n        }";
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        json,
        &unbounded_prio(),
    )
    .unwrap();
    let build = build_order(&arena, &unbounded_prio()).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let cfg = render_cfg(
        crate::OutputTemplate::Yaml,
        crate::serialization::types::Style::Default,
    );
    let out = render_top_k(&build, usize::MAX, &mut marks, 27, &cfg);
    assert_yaml_valid(&out);
    assert!(out.contains("normal: simple"));
    assert!(out.contains("\"010\": \"010\""));
    assert!(out.contains("\"a:b\": \"a:b\""));
    assert!(out.contains("\" spaced \": \" spaced \""));
    assert!(out.contains("reserved: \"yes\""));
    assert!(out.contains("multiline: \"line1\\nline2\""));
    assert!(out.contains("\"true\": 1"));
}

#[test]
fn string_parts_never_rendered_but_affect_truncation() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "\"abcdefghij\"",
        &unbounded_prio(),
    )
    .unwrap();
    let build = build_order(&arena, &unbounded_prio()).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let cfg = crate::RenderConfig {
        indent_unit: "".to_string(),
        newline: "".to_string(),
        ..render_cfg(
            crate::OutputTemplate::Json,
            crate::serialization::types::Style::Strict,
        )
    };
    let out = render_top_k(&build, 6, &mut marks, 99, &cfg);
    assert_eq!(out, "\"abcde…\"");
}

#[test]
fn yaml_array_of_objects_indentation() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[{\"a\":1,\"b\":2},{\"x\":3}]",
        &unbounded_prio(),
    )
    .unwrap();
    let build = build_order(&arena, &unbounded_prio()).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let cfg = render_cfg(
        crate::OutputTemplate::Yaml,
        crate::serialization::types::Style::Default,
    );
    let out = render_top_k(&build, usize::MAX, &mut marks, 28, &cfg);
    assert_yaml_valid(&out);
    assert!(out.contains("- a: 1") || out.contains("-   a: 1"));
    assert!(out.contains("  b: 2"));
}

#[test]
fn omitted_for_atomic_returns_none() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "1",
        &unbounded_prio(),
    )
    .unwrap();
    let build = build_order(&arena, &unbounded_prio()).unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let render_id = 7u32;
    marks[crate::order::ROOT_PQ_ID] = render_id;
    let cfg = crate::RenderConfig {
        indent_unit: "".to_string(),
        newline: "".to_string(),
        ..render_cfg(
            crate::OutputTemplate::Json,
            crate::serialization::types::Style::Strict,
        )
    };
    let leaf = super::leaf::LeafRenderer::new(&build, &cfg, None, |_id| None);
    let none = leaf.omitted_for(crate::order::ROOT_PQ_ID, 0);
    assert!(none.is_none());
}

#[test]
fn inline_open_array_in_object_json() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "{\"a\":[1,2,3]}",
        &crate::PriorityConfig::new(usize::MAX, 2),
    )
    .unwrap();
    let build =
        build_order(&arena, &crate::PriorityConfig::new(usize::MAX, 2))
            .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let cfg = render_cfg(
        crate::OutputTemplate::Json,
        crate::serialization::types::Style::Strict,
    );
    let out = render_top_k(&build, 4, &mut marks, 5, &cfg);
    assert_snapshot!("inline_open_array_in_object_json", out);
}

#[test]
fn arena_render_object_partial_js() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "{\"a\":1,\"b\":2,\"c\":3}",
        &unbounded_prio(),
    )
    .unwrap();
    let build = build_order(&arena, &unbounded_prio()).unwrap();
    let mut flags = vec![0u32; build.total_nodes];
    let cfg = crate::RenderConfig {
        color_mode: crate::ColorMode::Auto,
        ..render_cfg(
            crate::OutputTemplate::Js,
            crate::serialization::types::Style::Detailed,
        )
    };
    let out = render_top_k(&build, 2, &mut flags, 1, &cfg);
    assert!(out.starts_with("{\n"));
    assert!(
        out.contains("/* 2 more properties */"),
        "missing omitted summary: {out:?}"
    );
    assert!(
        out.contains("\"a\": 1")
            || out.contains("\"b\": 2")
            || out.contains("\"c\": 3")
    );
}

#[test]
fn array_internal_gaps_pseudo() {
    let ctx = mk_gap_ctx();
    let mut s = String::new();
    let cfg = render_cfg(
        crate::OutputTemplate::Pseudo,
        crate::serialization::types::Style::Default,
    );
    let mut outw = crate::serialization::output::Out::new(&mut s, &cfg, None);
    super::templates::render_array(
        crate::OutputTemplate::Pseudo,
        &ctx,
        &mut outw,
    );
    let out = s;
    assert_contains_all(
        &out,
        &["[\n", "\n  1,", "\n  …\n", "\n  2,", "\n  3\n"],
    );
}

#[test]
fn array_internal_gaps_js() {
    let ctx = mk_gap_ctx();
    let mut s = String::new();
    let cfg = render_cfg(
        crate::OutputTemplate::Js,
        crate::serialization::types::Style::Default,
    );
    let mut outw = crate::serialization::output::Out::new(&mut s, &cfg, None);
    super::templates::render_array(crate::OutputTemplate::Js, &ctx, &mut outw);
    let out = s;
    assert!(out.contains("/* 2 more items */"));
    assert!(out.contains("/* 1 more items */"));
}

#[test]
fn force_child_hooks_removed() {
    let order = PriorityOrder {
        metrics: vec![NodeMetrics::default(); 3],
        nodes: vec![
            RankedNode::Array {
                node_id: NodeId(0),
                key_in_object: None,
            },
            RankedNode::Array {
                node_id: NodeId(1),
                key_in_object: None,
            },
            RankedNode::Array {
                node_id: NodeId(2),
                key_in_object: None,
            },
        ],
        scores: vec![0, 0, 0],
        parent: vec![None, Some(NodeId(0)), Some(NodeId(0))],
        children: vec![vec![NodeId(1), NodeId(2)], Vec::new(), Vec::new()],
        index_in_parent_array: vec![None, Some(0), Some(1)],
        by_priority: vec![NodeId(0), NodeId(2), NodeId(1)],
        total_nodes: 3,
        object_type: vec![ObjectType::Object; 3],
        code_lines: HashMap::new(),
        fileset_children: None,
    };
    let mut flags = Vec::new();
    let render_id = 1u32;
    prepare_render_set_top_k_and_ancestors(&order, 1, &mut flags, render_id);
    assert_eq!(
        flags.get(1).copied().unwrap_or_default(),
        0,
        "force-first hooks removed: children should not be added when only the parent is selected"
    );
    assert_eq!(
        flags.get(2).copied().unwrap_or_default(),
        0,
        "force-first hooks removed: higher-priority siblings should also remain unselected"
    );
}

#[test]
fn fileset_tree_headers_free_keep_slot_stats_on_body_only() {
    let cfg_prio = crate::PriorityConfig::new(usize::MAX, usize::MAX);
    let arena = crate::ingest::fileset::parse_fileset_multi(
        vec![
            crate::ingest::fileset::FilesetInput {
                name: "a.txt".to_string(),
                bytes: b"line a\n".to_vec(),
                kind: crate::ingest::fileset::FilesetInputKind::Text {
                    atomic_lines: true,
                },
            },
            crate::ingest::fileset::FilesetInput {
                name: "b.txt".to_string(),
                bytes: b"line b\n".to_vec(),
                kind: crate::ingest::fileset::FilesetInputKind::Text {
                    atomic_lines: true,
                },
            },
        ],
        &cfg_prio,
    )
    .unwrap();
    let order = build_order(&arena, &cfg_prio).unwrap();
    let mut inclusion_flags = vec![0u32; order.total_nodes];
    prepare_render_set_top_k_and_ancestors(
        &order,
        usize::MAX,
        &mut inclusion_flags,
        1,
    );

    let slot_map =
        crate::pruner::budget::compute_fileset_slot_map(&order).unwrap();
    let slot_count = slot_map.iter().flatten().max().map(|s| *s + 1).unwrap();
    let recorder = crate::serialization::output::SlotStatsRecorder::new(
        slot_count, false,
    );

    let cfg = crate::RenderConfig {
        template: crate::OutputTemplate::Auto,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: crate::ColorMode::Off,
        color_enabled: false,
        style: crate::serialization::types::Style::Default,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        fileset_tree: true,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    };

    let (rendered, slot_stats) = render_from_render_set_with_slots(
        &order,
        &inclusion_flags,
        1,
        &cfg,
        Some(&slot_map),
        Some(recorder),
    );

    let stats = slot_stats.expect("slot stats present");
    assert_eq!(stats.len(), slot_count);
    assert!(
        stats[0].lines > 0 && stats[1].lines > 0,
        "measurement pass should count body lines per slot"
    );
    let rendered_lines = rendered.lines().count();
    let measured_lines: usize = stats.iter().map(|s| s.lines).sum();
    assert!(
        measured_lines < rendered_lines,
        "tree scaffolding should not consume per-slot budgets"
    );
    assert!(
        rendered.starts_with(".\n├"),
        "expected tree scaffold in user-facing render"
    );
}

#[test]
fn fileset_tree_headers_free_scaffold_does_not_change_slot_stats() {
    let cfg_prio = crate::PriorityConfig::new(usize::MAX, usize::MAX);
    let arena = crate::ingest::fileset::parse_fileset_multi(
        vec![
            crate::ingest::fileset::FilesetInput {
                name: "a.txt".to_string(),
                bytes: b"line a\n".to_vec(),
                kind: crate::ingest::fileset::FilesetInputKind::Text {
                    atomic_lines: true,
                },
            },
            crate::ingest::fileset::FilesetInput {
                name: "b.txt".to_string(),
                bytes: b"line b\n".to_vec(),
                kind: crate::ingest::fileset::FilesetInputKind::Text {
                    atomic_lines: true,
                },
            },
        ],
        &cfg_prio,
    )
    .unwrap();
    let order = build_order(&arena, &cfg_prio).unwrap();
    let mut inclusion_flags = vec![0u32; order.total_nodes];
    prepare_render_set_top_k_and_ancestors(
        &order,
        usize::MAX,
        &mut inclusion_flags,
        1,
    );

    let slot_map =
        crate::pruner::budget::compute_fileset_slot_map(&order).unwrap();
    let slot_count = slot_map.iter().flatten().max().map(|s| *s + 1).unwrap();

    let base_cfg = crate::RenderConfig {
        template: crate::OutputTemplate::Auto,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: crate::ColorMode::Off,
        color_enabled: false,
        style: crate::serialization::types::Style::Default,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        fileset_tree: true,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    };

    let render_with_scaffold = |show_headers: bool| {
        let cfg = crate::RenderConfig {
            show_fileset_headers: show_headers,
            ..base_cfg.clone()
        };
        let recorder = crate::serialization::output::SlotStatsRecorder::new(
            slot_count, false,
        );
        render_from_render_set_with_slots(
            &order,
            &inclusion_flags,
            1,
            &cfg,
            Some(&slot_map),
            Some(recorder),
        )
    };

    let (with_scaffold_render, with_scaffold_stats) =
        render_with_scaffold(true);
    let (without_scaffold_render, without_scaffold_stats) =
        render_with_scaffold(false);

    assert_ne!(
        with_scaffold_render, without_scaffold_render,
        "outputs should differ when scaffolding is toggled"
    );
    let with_stats = with_scaffold_stats.expect("slot stats present");
    let without_stats =
        without_scaffold_stats.expect("slot stats present without scaffold");
    assert_eq!(
        with_stats, without_stats,
        "per-slot counts should ignore scaffolding when headers are free"
    );
}
