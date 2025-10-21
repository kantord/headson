#[path = "../test_support/mod.rs"]
mod util;
use std::fs;

fn run_object_case(template: &str, budget: usize, extra: &[&str]) -> String {
    let s = fs::read_to_string("tests/fixtures/explicit/object_small.json")
        .expect("read fixture");
    util::run_template_budget(&s, template, budget, extra)
}

fn parse_js_object_omitted(out_js: &str) -> usize {
    let trimmed = out_js.trim_end_matches(['\r', '\n']).trim();
    assert!(
        trimmed.starts_with('{') && trimmed.ends_with('}'),
        "unexpected shape: {out_js:?}"
    );
    let body = &trimmed[1..trimmed.len() - 1];
    let (_, comment) = body.split_once("/*").expect("has comment");
    let digits: String =
        comment.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse::<usize>().expect("parse omitted")
}

#[test]
fn object_truncated_js_kept0_reports_omitted_count() {
    // Root object has 2 properties in the fixture. With a tiny budget,
    // include root only (kept=0), omitted should be 2.
    let out_js = run_object_case("js", 30, &["--compact"]);
    let omitted = parse_js_object_omitted(&out_js);
    assert_eq!(omitted, 2);
}

#[test]
fn object_truncated_pseudo_has_ellipsis() {
    let out = run_object_case("pseudo", 10, &[]);
    assert!(out.starts_with('{') && out.ends_with("}\n"));
    assert!(out.contains('…'));
}

#[test]
fn object_truncated_json_is_empty_object() {
    let out = run_object_case("json", 10, &[]);
    assert_eq!(out, "{}\n");
}
