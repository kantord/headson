mod common;
use insta::assert_snapshot;

fn run_yaml_stdin(input: &str) -> String {
    common::run_template_budget_no_color(
        input,
        "yaml",
        100_000,
        &["-i", "yaml"],
    )
}

#[test]
fn yaml_snapshot_basic_stdin() {
    let y = "a: 1\narr:\n  - x\n  - y\n";
    let out = run_yaml_stdin(y);
    assert_snapshot!("yaml_snapshot_basic_stdin", out);
}

#[test]
fn yaml_snapshot_multidoc_stdin() {
    let y = "---\na: 1\n---\n- z\n";
    let out = run_yaml_stdin(y);
    assert_snapshot!("yaml_snapshot_multidoc_stdin", out);
}

#[test]
fn yaml_snapshot_json_input_quoting_digit_key() {
    // JSON input rendered as YAML; exercises key/value quoting for numeric-like tokens
    let j = r#"{"010": "010"}"#;
    let out = common::run_template_budget_no_color(j, "yaml", 100_000, &[]);
    assert_snapshot!("yaml_snapshot_json_input_quoting_digit_key", out);
}

#[test]
fn yaml_snapshot_json_input_reserved_value() {
    // JSON input rendered as YAML; reserved word value should be quoted
    let j = r#"{"reserved": "yes"}"#;
    let out = common::run_template_budget_no_color(j, "yaml", 100_000, &[]);
    assert_snapshot!("yaml_snapshot_json_input_reserved_value", out);
}
