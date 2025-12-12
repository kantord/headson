use super::*;
use crate::order::types::NodeMetrics;
use crate::order::{
    NodeId, ObjectType, PriorityOrder, RankedNode, build_order,
};
use insta::assert_snapshot;

fn render_top_k(
    order_build: &PriorityOrder,
    top_k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
    config: &crate::RenderConfig,
) -> String {
    prepare_render_set_top_k_and_ancestors(
        order_build,
        top_k,
        inclusion_flags,
        render_id,
    );
    render_from_render_set_with_slots_impl(
        order_build,
        inclusion_flags,
        render_id,
        config,
        None,
        None,
        true,
    )
    .0
}

fn assert_yaml_valid(s: &str) {
    let _: serde_yaml::Value =
        serde_yaml::from_str(s).expect("YAML parse failed (validation)");
}

#[test]
fn arena_render_empty_array() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[]",
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let build = build_order(
        &arena,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let out = render_top_k(
        &build,
        10,
        &mut marks,
        1,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Json,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Auto,
            color_enabled: false,
            style: crate::serialization::types::Style::Strict,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_snapshot!("arena_render_empty", out);
}

#[test]
fn newline_detection_crlf_array_child() {
    // Ensure we exercise the render_has_newline branch that checks
    // arbitrary newline sequences (e.g., "\r\n") via s.contains(nl).
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[{\"a\":1,\"b\":2}]",
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let build = build_order(
        &arena,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let out = render_top_k(
        &build,
        usize::MAX,
        &mut marks,
        1,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Json,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            // Use CRLF to force the contains(nl) path.
            newline: "\r\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Auto,
            color_enabled: false,
            style: crate::serialization::types::Style::Strict,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    // Sanity: output should contain CRLF newlines and render the object child across lines.
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
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let build = build_order(
        &arena,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let out = render_top_k(
        &build,
        10,
        &mut marks,
        1,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Json,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Auto,
            color_enabled: false,
            style: crate::serialization::types::Style::Strict,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_snapshot!("arena_render_single", out);
}

#[test]
fn array_omitted_markers_pseudo_head_and_tail() {
    // Force sampling to keep only a subset so omitted > 0.
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

    // Head preference: omitted marker after items.
    let out_head = render_top_k(
        &build,
        2,
        &mut marks,
        1,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Pseudo,
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
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_snapshot!("array_omitted_pseudo_head", out_head);

    // Tail preference: omitted marker before items (with comma).
    let out_tail = render_top_k(
        &build,
        2,
        &mut marks,
        2,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Pseudo,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: true,
            color_mode: crate::ColorMode::Off,
            color_enabled: false,
            style: crate::serialization::types::Style::Default,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
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

    let out_head = render_top_k(
        &build,
        2,
        &mut marks,
        3,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Js,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Off,
            color_enabled: false,
            style: crate::serialization::types::Style::Detailed,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_snapshot!("array_omitted_js_head", out_head);

    let out_tail = render_top_k(
        &build,
        2,
        &mut marks,
        4,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Js,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: true,
            color_mode: crate::ColorMode::Off,
            color_enabled: false,
            style: crate::serialization::types::Style::Detailed,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
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

    let out_head = render_top_k(
        &build,
        2,
        &mut marks,
        11,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Yaml,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Off,
            color_enabled: false,
            style: crate::serialization::types::Style::Detailed,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_yaml_valid(&out_head);
    assert_snapshot!("array_omitted_yaml_head", out_head);

    let out_tail = render_top_k(
        &build,
        2,
        &mut marks,
        12,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Yaml,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: true,
            color_mode: crate::ColorMode::Off,
            color_enabled: false,
            style: crate::serialization::types::Style::Detailed,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_yaml_valid(&out_tail);
    assert_snapshot!("array_omitted_yaml_tail", out_tail);
}

#[test]
fn arena_render_empty_array_yaml() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[]",
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let build = build_order(
        &arena,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let out = render_top_k(
        &build,
        10,
        &mut marks,
        21,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Yaml,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Auto,
            color_enabled: false,
            style: crate::serialization::types::Style::Default,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_yaml_valid(&out);
    assert_snapshot!("arena_render_empty_yaml", out);
}

#[test]
fn arena_render_single_string_array_yaml() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[\"ab\"]",
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let build = build_order(
        &arena,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let out = render_top_k(
        &build,
        10,
        &mut marks,
        22,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Yaml,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Auto,
            color_enabled: false,
            style: crate::serialization::types::Style::Default,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
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
    let out = render_top_k(
        &build,
        4,
        &mut marks,
        23,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Yaml,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Off,
            color_enabled: false,
            style: crate::serialization::types::Style::Detailed,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_yaml_valid(&out);
    assert_snapshot!("inline_open_array_in_object_yaml", out);
}

#[test]
fn array_internal_gaps_yaml() {
    let ctx = mk_gap_ctx();
    let mut s = String::new();
    let cfg = test_render_cfg(
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
    // Keys and values that exercise YAML quoting heuristics.
    let json = "{\n            \"true\": 1,\n            \"010\": \"010\",\n            \"-dash\": \"ok\",\n            \"normal\": \"simple\",\n            \"a:b\": \"a:b\",\n            \" spaced \": \" spaced \",\n            \"reserved\": \"yes\",\n            \"multiline\": \"line1\\nline2\"\n        }";
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        json,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let build = build_order(
        &arena,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let out = render_top_k(
        &build,
        usize::MAX,
        &mut marks,
        27,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Yaml,
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
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_yaml_valid(&out);
    // Unquoted safe key
    assert!(
        out.contains("normal: simple"),
        "expected unquoted normal key/value: {out:?}"
    );
    // Quoted key starting with digit and quoted numeric-looking value
    assert!(
        out.contains("\"010\": \"010\""),
        "expected quoted numeric-like key and value: {out:?}"
    );
    // Quoted key with punctuation ':' and quoted value with ':'
    assert!(
        out.contains("\"a:b\": \"a:b\""),
        "expected quoted punctuated key/value: {out:?}"
    );
    // Quoted key/value with outer whitespace
    assert!(
        out.contains("\" spaced \": \" spaced \""),
        "expected quotes for outer whitespace: {out:?}"
    );
    // Reserved word value quoted
    assert!(
        out.contains("reserved: \"yes\""),
        "expected reserved word value quoted: {out:?}"
    );
    // Multiline string stays quoted and appears on a single line token here
    assert!(
        out.contains("multiline: \"line1\\nline2\""),
        "expected JSON-escaped newline token for strings: {out:?}"
    );
    // Key 'true' must be quoted to avoid YAML boolean
    assert!(
        out.contains("\"true\": 1"),
        "expected quoted boolean-like key: {out:?}"
    );
}

#[test]
fn string_parts_never_rendered_but_affect_truncation() {
    // Build a long string: the string node itself is SplittableLeaf; the
    // builder also creates LeafPart children used only for priority.
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "\"abcdefghij\"",
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let build = build_order(
        &arena,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    // Include the root string node plus 5 grapheme parts (total top_k = 1 + 5).
    let out = render_top_k(
        &build,
        6,
        &mut marks,
        99,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Json,
            indent_unit: "".to_string(),
            space: " ".to_string(),
            newline: "".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Off,
            color_enabled: false,
            style: crate::serialization::types::Style::Strict,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    // Expect the first 5 characters plus an ellipsis, as a valid JSON string literal.
    assert_eq!(out, "\"abcde…\"");
}

#[test]
fn yaml_array_of_objects_indentation() {
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "[{\"a\":1,\"b\":2},{\"x\":3}]",
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let build = build_order(
        &arena,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let out = render_top_k(
        &build,
        usize::MAX,
        &mut marks,
        28,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Yaml,
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
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_yaml_valid(&out);
    // Expect dash-prefixed first line and continued indentation for following lines
    assert!(
        out.contains("- a: 1") || out.contains("-   a: 1"),
        "expected list dash with first object line: {out:?}"
    );
    assert!(
        out.contains("  b: 2"),
        "expected subsequent object key indented: {out:?}"
    );
}

#[test]
fn omitted_for_atomic_returns_none() {
    // Single atomic value as input (number), root is AtomicLeaf.
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "1",
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let build = build_order(
        &arena,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let mut marks = vec![0u32; build.total_nodes];
    let render_id = 7u32;
    // Mark the root included for this render set.
    marks[crate::order::ROOT_PQ_ID] = render_id;
    let cfg = crate::RenderConfig {
        template: crate::OutputTemplate::Json,
        indent_unit: "".to_string(),
        space: " ".to_string(),
        newline: "".to_string(),
        prefer_tail_arrays: false,
        color_mode: crate::ColorMode::Off,
        color_enabled: false,
        style: crate::serialization::types::Style::Strict,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        fileset_tree: false,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    };
    let scope = RenderScope {
        order: &build,
        inclusion_flags: &marks,
        render_set_id: render_id,
        config: &cfg,
        line_number_width: None,
        code_highlight_cache: HashMap::new(),
        grep_highlight: None,
        slot_map: None,
    };
    // Atomic leaves never report omitted counts.
    let none = scope.omitted_for(crate::order::ROOT_PQ_ID, 0);
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
    let out = render_top_k(
        &build,
        4,
        &mut marks,
        5,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Json,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Off,
            color_enabled: false,
            style: crate::serialization::types::Style::Strict,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    assert_snapshot!("inline_open_array_in_object_json", out);
}

#[test]
fn arena_render_object_partial_js() {
    // Object with three properties; render top_k small so only one child is kept.
    let arena = crate::ingest::formats::json::build_json_tree_arena(
        "{\"a\":1,\"b\":2,\"c\":3}",
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let build = build_order(
        &arena,
        &crate::PriorityConfig::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let mut flags = vec![0u32; build.total_nodes];
    // top_k=2 → root object + first property
    let out = render_top_k(
        &build,
        2,
        &mut flags,
        1,
        &crate::RenderConfig {
            template: crate::OutputTemplate::Js,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Auto,
            color_enabled: false,
            style: crate::serialization::types::Style::Detailed,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        },
    );
    // Should be a valid JS object with one property and an omitted summary.
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

fn test_render_cfg(
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
fn array_internal_gaps_pseudo() {
    let ctx = mk_gap_ctx();
    let mut s = String::new();
    let cfg = test_render_cfg(
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
    let cfg = test_render_cfg(
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
    // Parent has two children; child with PQ id 2 has higher global priority
    // than child with PQ id 1, but force-first-child currently pulls the
    // first listed child. This captures the undesired behavior.
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
        children: vec![
            vec![NodeId(1), NodeId(2)], // first child = NodeId(1)
            Vec::new(),
            Vec::new(),
        ],
        index_in_parent_array: vec![None, Some(0), Some(1)],
        by_priority: vec![NodeId(0), NodeId(2), NodeId(1)], // child 2 outranks child 1
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
