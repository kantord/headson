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
fn array_tail_pseudo_ellipsis_at_start() {
    // Force omissions with a small budget, and enable tail mode.
    let budget = 30usize;
    let out = run_array_case("pseudo", budget, &["--tail"]);
    // In compact mode, the omission marker should appear immediately after '['.
    assert!(
        out.starts_with("[…]".trim_end_matches(']')) || out.starts_with("[…"),
        "expected output to start with '[…' in tail mode (pseudo): {out:?}"
    );
}

#[test]
fn array_tail_js_comment_first() {
    let budget = 30usize;
    let out = run_array_case("js", budget, &["--tail"]);
    // In compact mode, the omission comment should immediately follow '['.
    assert!(
        out.starts_with("[/*"),
        "expected output to start with '[/*' in tail mode (js): {out:?}"
    );
}

#[test]
fn array_tail_pseudo_leading_marker_has_comma() {
    // Non-compact to inspect individual lines; expect comma after leading ellipsis.
    let s =
        fs::read_to_string("tests/fixtures/explicit/array_numbers_50.json")
            .expect("read fixture");
    let out = util::run_template_budget(&s, "pseudo", 40, &["--tail"]);
    assert!(
        out.contains("\n  …,\n"),
        "expected leading ellipsis with trailing comma in pseudo: {out:?}"
    );
}

#[test]
fn array_tail_js_leading_marker_has_comma() {
    // Non-compact; leading JS omission comment should end with a comma when items follow.
    let s =
        fs::read_to_string("tests/fixtures/explicit/array_numbers_50.json")
            .expect("read fixture");
    let out = util::run_template_budget(&s, "js", 40, &["--tail"]);
    assert!(
        out.contains("\n  /*") && out.contains("*/,\n"),
        "expected trailing comma after omission comment in js: {out:?}"
    );
}

#[test]
fn array_tail_json_contains_last_k_values() {
    // Build a simple 0..49 array and ensure tail keeps the last K in JSON.
    let values: Vec<String> = (0..50).map(|i| i.to_string()).collect();
    let input = format!("[{}]", values.join(","));
    let render_cfg = headson::RenderConfig {
        template: headson::OutputTemplate::Json,
        indent_unit: "  ".into(),
        space: " ".into(),
        newline: "\n".into(),
        prefer_tail_arrays: true,
    };
    let mut prio = headson::PriorityConfig::new(usize::MAX, 15);
    prio.prefer_tail_arrays = true;
    prio.array_sampler = headson::ArraySamplerStrategy::Tail;
    let out = headson::headson(input.into_bytes(), &render_cfg, &prio, 10_000)
        .expect("render");
    let v: serde_json::Value = serde_json::from_str(&out).expect("json parse");
    let arr = v.as_array().expect("root array");
    assert_eq!(arr.len(), 15, "kept exactly cap items");
    let first = arr.first().and_then(serde_json::Value::as_u64).unwrap();
    let last = arr.last().and_then(serde_json::Value::as_u64).unwrap();
    assert_eq!(first, 35);
    assert_eq!(last, 49);
}
