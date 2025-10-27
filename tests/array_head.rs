#[path = "../test_support/mod.rs"]
mod util;
use std::fs;

fn run_array_case(template: &str, budget: usize, extra: &[&str]) -> String {
    let s =
        fs::read_to_string("tests/fixtures/explicit/array_numbers_50.json")
            .expect("read fixture");
    let mut args = vec!["--compact"];
    args.extend_from_slice(extra);
    util::run_template_budget(&s, template, budget, &args)
}

#[test]
fn array_head_pseudo_ellipsis_at_end() {
    let budget = 30usize;
    let out = run_array_case("pseudo", budget, &["--head"]);
    let out = out.trim_end_matches(['\r', '\n']).to_string();
    // In compact mode, the omission marker should appear just before ']'.
    assert!(
        out.ends_with("]") && (out.contains("…]") || out.contains("…,]")),
        "expected output to end with '…]' in head mode (pseudo): {out:?}"
    );
    // Ensure no leading omission marker at the start for head mode.
    if let Some(open_idx) = out.lines().position(|l| l.trim() == "[") {
        let first_non_empty = out
            .lines()
            .skip(open_idx + 1)
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        assert!(
            !first_non_empty.trim().starts_with('…'),
            "head mode should not have leading ellipsis: {out:?}"
        );
    }
}

#[test]
fn array_head_js_comment_last() {
    let budget = 30usize;
    let out = run_array_case("js", budget, &["--head"]);
    let out = out.trim_end_matches(['\r', '\n']).to_string();
    assert!(
        out.ends_with("]") && out.contains("/*") && out.contains("*/]"),
        "expected omission comment at end in head mode (js): {out:?}"
    );
    // Ensure no leading omission comment at the start for head mode.
    if let Some(open_idx) = out.lines().position(|l| l.trim() == "[") {
        let first_non_empty = out
            .lines()
            .skip(open_idx + 1)
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        assert!(
            !first_non_empty.trim().starts_with("/*"),
            "head mode should not have leading omission comment: {out:?}"
        );
    }
}

#[test]
fn array_head_json_contains_first_k_values() {
    // Build a simple 0..49 array and ensure head keeps the first K in JSON.
    let values: Vec<String> = (0..50).map(|i| i.to_string()).collect();
    let input = format!("[{}]", values.join(","));
    let render_cfg = headson::RenderConfig {
        template: headson::OutputTemplate::Json,
        indent_unit: "  ".into(),
        space: " ".into(),
        newline: "\n".into(),
        prefer_tail_arrays: false,
        color_mode: headson::ColorMode::Auto,
        color_enabled: false,
    };
    let mut prio = headson::PriorityConfig::new(usize::MAX, 15);
    prio.prefer_tail_arrays = false;
    prio.array_sampler = headson::ArraySamplerStrategy::Head;
    let out = headson::headson(input.into_bytes(), &render_cfg, &prio, 10_000)
        .expect("render");
    let v: serde_json::Value = serde_json::from_str(&out).expect("json parse");
    let arr = v.as_array().expect("root array");
    assert_eq!(arr.len(), 15, "kept exactly cap items");
    let first = arr.first().and_then(serde_json::Value::as_u64).unwrap();
    let last = arr.last().and_then(serde_json::Value::as_u64).unwrap();
    assert_eq!(first, 0);
    assert_eq!(last, 14);
}
