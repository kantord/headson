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

fn parse_js_kept_omitted(out_js: &str) -> (usize, usize) {
    assert!(out_js.starts_with('[') && out_js.ends_with("]\n"));
    let body = &out_js[1..out_js.len() - 2];
    let (left, comment) = body.split_once("/*").expect("has comment");
    let kept = if left.trim().is_empty() {
        0
    } else {
        left.bytes().filter(|&b| b == b',').count() + 1
    };
    let digits: String =
        comment.chars().filter(char::is_ascii_digit).collect();
    let omitted = digits.parse::<usize>().expect("parse omitted");
    (kept, omitted)
}

#[test]
fn array_truncated_js_kept_plus_omitted_equals_total() {
    let len = 50usize;
    let budget = 30usize; // parse cap = 15
    let out_js = run_array_case("js", budget, &[]);
    let (kept, omitted) = parse_js_kept_omitted(&out_js);
    assert_eq!(kept + omitted, len, "kept+omitted must equal total");
}

#[test]
fn array_truncated_pseudo_has_ellipsis() {
    let budget = 30usize;
    let out_pseudo = run_array_case("pseudo", budget, &[]);
    assert!(out_pseudo.starts_with('[') && out_pseudo.ends_with("]\n"));
    assert!(
        out_pseudo.contains('…'),
        "expected ellipsis: {out_pseudo:?}"
    );
}

#[test]
fn array_truncated_json_length_within_cap() {
    let budget = 30usize;
    let out_json = run_array_case("json", budget, &[]);
    let v: serde_json::Value =
        serde_json::from_str(&out_json).expect("json parse");
    let arr = v.as_array().expect("root array");
    assert!(
        arr.len() <= budget / 2,
        "array length should be <= parse cap"
    );
}
