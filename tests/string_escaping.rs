#[path = "../test_support/mod.rs"]
mod util;
use std::fs;

fn load_fixture() -> String {
    fs::read_to_string("tests/fixtures/explicit/string_escaping.json")
        .expect("read fixture")
}

fn has_all_escapes(s: &str) -> bool {
    s.contains("\\\"")
        && s.contains("\\\\")
        && s.contains("\\n")
        && s.contains("\\t")
        && s.contains("\\u0000")
}

fn assert_full_output_ok(template: &str) {
    let input = load_fixture();
    let out = util::run_template_budget(&input, template, 10_000, &[]);
    let trimmed = out.trim_end_matches(['\r', '\n']).to_string();
    assert!(
        trimmed.starts_with('"') && trimmed.ends_with('"'),
        "quoted string: {trimmed:?}"
    );
    assert!(
        has_all_escapes(&trimmed),
        "expected escapes in: {trimmed:?}"
    );
    let parsed: String =
        serde_json::from_str(&trimmed).expect("parse json string");
    let original: String =
        serde_json::from_str(&input).expect("parse fixture json string");
    assert_eq!(parsed, original, "roundtrip equality for tmpl={template}");
}

#[test]
fn escaping_preserved_in_full_output() {
    for &tmpl in &["json", "pseudo", "js"] {
        assert_full_output_ok(tmpl);
    }
}

fn assert_truncated_output_ok(template: &str) {
    let input = load_fixture();
    let out = util::run_template_budget(&input, template, 20, &[]);
    let trimmed = out.trim_end_matches(['\r', '\n']).to_string();
    assert!(
        trimmed.starts_with('"') && trimmed.ends_with('"'),
        "quoted truncated string: {trimmed:?}"
    );
    assert!(
        trimmed.contains('…'),
        "ellipsis present in truncated output: {trimmed:?}"
    );
    // It should remain valid JSON and parse to a Rust String ending with an ellipsis.
    let parsed: String =
        serde_json::from_str(&trimmed).expect("parse truncated json string");
    assert!(
        parsed.ends_with('…'),
        "parsed truncated string should end with ellipsis: {parsed:?}"
    );
}

#[test]
fn escaping_and_ellipsis_in_truncated_output() {
    for &tmpl in &["json", "pseudo", "js"] {
        assert_truncated_output_ok(tmpl);
    }
}
