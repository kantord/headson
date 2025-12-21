mod common;
use common::run_template_budget_assert_no_color;

#[test]
fn auto_on_stdin_defaults_to_json_output() {
    let input = "{\"a\":1}";
    let assert = run_template_budget_assert_no_color(input, "auto", 1000, &[])
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    // JSON output should start with '{' and be a single valid JSON object
    assert!(
        out.trim_start().starts_with('{'),
        "expected JSON output: {out:?}"
    );
}
