mod common;

use insta::assert_snapshot;

fn basic_input() -> &'static str {
    concat!(
        r#"{"id": 1, "name": "Alice", "email": "alice@example.com"}"#,
        "\n",
        r#"{"id": 2, "name": "Bob", "email": "bob@example.com"}"#,
        "\n",
        r#"{"id": 3, "name": "Charlie", "email": "charlie@example.com"}"#,
        "\n",
        r#"{"id": 4, "name": "Diana", "email": "diana@example.com"}"#,
        "\n",
        r#"{"id": 5, "name": "Eve", "email": "eve@example.com"}"#,
        "\n",
    )
}

fn empty_lines_input() -> &'static str {
    concat!(
        r#"{"id": 1, "name": "Alice"}"#,
        "\n",
        "\n",
        r#"{"id": 2, "name": "Bob"}"#,
        "\n",
        "\n",
        r#"{"id": 3, "name": "Charlie"}"#,
        "\n",
    )
}

fn large_input() -> String {
    let mut s = (0..50)
        .map(|i| format!(r#"{{"id": {i}}}"#))
        .collect::<Vec<_>>()
        .join("\n");
    s.push('\n');
    s
}

// ---------------------------------------------------------------------------
// A. Basic parsing (stdin with -i jsonl)
// ---------------------------------------------------------------------------

#[test]
fn jsonl_basic_default() {
    let out = common::run_template_budget_no_color(
        basic_input(),
        "pseudo",
        10000,
        &["-i", "jsonl"],
    );
    assert_snapshot!(out);
}

#[test]
fn jsonl_basic_strict() {
    let out = common::run_template_budget_no_color(
        basic_input(),
        "json",
        10000,
        &["-i", "jsonl"],
    );
    // Strict must produce valid JSON
    let v: serde_json::Value = serde_json::from_str(&out)
        .expect("strict output should be valid JSON");
    assert!(v.is_array(), "strict output should be an array");
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 5, "all 5 entries should be present");
    assert_snapshot!(out);
}

#[test]
fn jsonl_basic_detailed() {
    let out = common::run_template_budget_no_color(
        basic_input(),
        "js",
        10000,
        &["-i", "jsonl"],
    );
    assert_snapshot!(out);
}

// ---------------------------------------------------------------------------
// B. Line numbers in auto/default/detailed output
// ---------------------------------------------------------------------------

#[test]
fn jsonl_default_shows_line_numbers() {
    let out = common::run_template_budget_no_color(
        basic_input(),
        "pseudo",
        10000,
        &["-i", "jsonl"],
    );
    // Default/pseudo template should show line number prefixes
    assert!(out.contains("1:"), "should contain line number prefix '1:'");
    assert!(out.contains("2:"), "should contain line number prefix '2:'");
}

#[test]
fn jsonl_strict_no_line_numbers() {
    let out = common::run_template_budget_no_color(
        basic_input(),
        "json",
        10000,
        &["-i", "jsonl"],
    );
    // Strict output should not contain line number prefixes
    // It should start with '[' (valid JSON array)
    let trimmed = out.trim();
    assert!(
        trimmed.starts_with('['),
        "strict output should start with '[', got: {trimmed:?}"
    );
}

// ---------------------------------------------------------------------------
// C. Empty line handling
// ---------------------------------------------------------------------------

#[test]
fn jsonl_skips_empty_lines() {
    let out = common::run_template_budget_no_color(
        empty_lines_input(),
        "pseudo",
        10000,
        &["-i", "jsonl"],
    );
    // Should have 3 entries (empty lines skipped)
    // Line numbers should reflect original file positions: 1, 3, 5
    assert!(out.contains("1:"), "should show line 1");
    assert!(
        out.contains("3:"),
        "should show line 3 (after empty line 2)"
    );
    assert!(
        out.contains("5:"),
        "should show line 5 (after empty line 4)"
    );
    // Should NOT contain line 2 or 4 (those are empty)
    assert_snapshot!(out);
}

// ---------------------------------------------------------------------------
// D. Truncation / sampling
// ---------------------------------------------------------------------------

#[test]
fn jsonl_truncation_default() {
    let input = large_input();
    let out = common::run_template_budget_no_color(
        &input,
        "pseudo",
        200,
        &["-i", "jsonl"],
    );
    // With 50 entries and small budget, should show omission marker
    assert_snapshot!(out);
}

#[test]
fn jsonl_truncation_strict_valid_json() {
    let input = large_input();
    let out = common::run_template_budget_no_color(
        &input,
        "json",
        200,
        &["-i", "jsonl"],
    );
    let v: serde_json::Value = serde_json::from_str(&out)
        .expect("truncated strict output should be valid JSON");
    assert!(v.is_array(), "truncated strict output should be an array");
    let arr = v.as_array().unwrap();
    assert!(
        arr.len() < 50,
        "should be truncated to fewer than 50 entries"
    );
}

#[test]
fn jsonl_head_mode() {
    let input = large_input();
    let out = common::run_template_budget_no_color(
        &input,
        "json",
        500,
        &["-i", "jsonl", "--head"],
    );
    let v: serde_json::Value = serde_json::from_str(&out).expect("json parse");
    let arr = v.as_array().expect("root array");
    assert!(!arr.is_empty(), "should have some entries");
    // First entry should be id: 0
    let first_id = arr[0]
        .get("id")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    assert_eq!(first_id, 0, "head mode should keep first entry");
}

#[test]
fn jsonl_tail_mode() {
    let input = large_input();
    let out = common::run_template_budget_no_color(
        &input,
        "json",
        500,
        &["-i", "jsonl", "--tail"],
    );
    let v: serde_json::Value = serde_json::from_str(&out).expect("json parse");
    let arr = v.as_array().expect("root array");
    assert!(!arr.is_empty(), "should have some entries");
    // Last entry should be id: 49
    let last_entry = arr.last().unwrap();
    let last_id = last_entry
        .get("id")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    assert_eq!(last_id, 49, "tail mode should keep last entry");
}

// ---------------------------------------------------------------------------
// E. File-based input (auto-detection from .jsonl extension)
// ---------------------------------------------------------------------------

#[test]
fn jsonl_file_auto_detect() {
    let path = "tests/fixtures/explicit/basic.jsonl";
    let out = common::run_cli(
        &[
            "--no-color",
            "-c",
            "10000",
            "-f",
            "json",
            "-t",
            "strict",
            path,
        ],
        None,
    );
    let v: serde_json::Value = serde_json::from_str(&out.stdout)
        .expect("file-based jsonl should produce valid JSON");
    assert!(v.is_array(), "should be an array");
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 5, "should have 5 entries");
}

#[test]
fn jsonl_file_auto_detect_default_template() {
    let path = "tests/fixtures/explicit/basic.jsonl";
    let out = common::run_cli(
        &[
            "--no-color",
            "-c",
            "10000",
            "-f",
            "json",
            "-t",
            "default",
            path,
        ],
        None,
    );
    // Auto-detected JSONL with default template should show line numbers
    assert!(
        out.stdout.contains("1:"),
        "file-based jsonl with default template should show line numbers"
    );
    assert_snapshot!("jsonl_file_auto_detect_default", out.stdout);
}

// ---------------------------------------------------------------------------
// F. YAML output
// ---------------------------------------------------------------------------

#[test]
fn jsonl_yaml_output() {
    let out = common::run_template_budget_no_color(
        basic_input(),
        "yaml",
        10000,
        &["-i", "jsonl"],
    );
    // YAML output should use multi-document mode (--- separators), not array syntax
    assert!(
        out.contains("---"),
        "YAML output should use multi-document separators"
    );
    assert!(
        !out.contains("- id:"),
        "YAML output should not use array item syntax for JSONL root"
    );
    assert!(out.contains("id:"), "should render as YAML");
    assert_snapshot!(out);
}

// ---------------------------------------------------------------------------
// G. Compact mode
// ---------------------------------------------------------------------------

#[test]
fn jsonl_compact_default() {
    let out = common::run_template_budget_no_color(
        basic_input(),
        "pseudo",
        10000,
        &["-i", "jsonl", "--compact"],
    );
    // Compact should have no indentation but still be JSONL-style
    assert!(!out.contains('\n') || out.lines().count() <= 6);
    assert_snapshot!(out);
}

// ---------------------------------------------------------------------------
// H. Single-line JSONL
// ---------------------------------------------------------------------------

#[test]
fn jsonl_single_line() {
    let input = concat!(r#"{"id": 1, "name": "Alice"}"#, "\n");
    let out = common::run_template_budget_no_color(
        input,
        "pseudo",
        10000,
        &["-i", "jsonl"],
    );
    // Single entry should still render in JSONL style (no array wrapper)
    let trimmed = out.trim();
    assert!(
        !trimmed.starts_with('['),
        "single-line JSONL with default template should not have array wrapper"
    );
    assert_snapshot!(out);
}

// ---------------------------------------------------------------------------
// I. Trailing newline
// ---------------------------------------------------------------------------

#[test]
fn jsonl_trailing_newline() {
    // Input with trailing newline should not produce an extra empty entry
    let with_trailing = r#"{"a": 1}
{"b": 2}
"#;
    let without_trailing = "{\"a\": 1}\n{\"b\": 2}";
    let out_with = common::run_template_budget_no_color(
        with_trailing,
        "json",
        10000,
        &["-i", "jsonl"],
    );
    let out_without = common::run_template_budget_no_color(
        without_trailing,
        "json",
        10000,
        &["-i", "jsonl"],
    );
    // Both should produce the same output
    assert_eq!(
        out_with, out_without,
        "trailing newline should not add an extra entry"
    );
    // Both should have exactly 2 entries
    let v: serde_json::Value =
        serde_json::from_str(&out_with).expect("json parse");
    let arr = v.as_array().expect("root array");
    assert_eq!(arr.len(), 2, "should have exactly 2 entries");
}

// ---------------------------------------------------------------------------
// J. Empty lines file fixture
// ---------------------------------------------------------------------------

#[test]
fn jsonl_file_with_empty_lines() {
    let path = "tests/fixtures/explicit/with_empty_lines.jsonl";
    let out = common::run_cli(
        &[
            "--no-color",
            "-c",
            "10000",
            "-f",
            "json",
            "-t",
            "default",
            path,
        ],
        None,
    );
    // Should show original line numbers (1, 3, 5) since lines 2 and 4 are empty
    assert!(out.stdout.contains("1:"), "should show original line 1");
    assert!(out.stdout.contains("3:"), "should show original line 3");
    assert!(out.stdout.contains("5:"), "should show original line 5");
    assert_snapshot!("jsonl_file_with_empty_lines", out.stdout);
}

// ---------------------------------------------------------------------------
// K. Grep + JSONL
// ---------------------------------------------------------------------------

#[test]
fn jsonl_grep_keeps_matching_line_with_tiny_budget() {
    let input = concat!(
        r#"{"id": 1, "name": "Alice"}"#,
        "\n",
        r#"{"id": 2, "name": "Bob"}"#,
        "\n",
        r#"{"id": 3, "name": "needle"}"#,
        "\n",
        r#"{"id": 4, "name": "Diana"}"#,
        "\n",
    );
    let out = common::run_cli(
        &[
            "--no-color",
            "--bytes",
            "10",
            "-f",
            "json",
            "-t",
            "strict",
            "-i",
            "jsonl",
            "--grep",
            "needle",
        ],
        Some(input.as_bytes()),
    );
    assert!(
        out.stdout.contains("needle"),
        "strong grep should guarantee matched JSONL line is present even with tiny budget; got: {:?}",
        out.stdout
    );
}

#[test]
fn jsonl_grep_preserves_line_numbers() {
    let input = concat!(
        r#"{"id": 1, "val": "skip"}"#,
        "\n",
        r#"{"id": 2, "val": "needle"}"#,
        "\n",
        r#"{"id": 3, "val": "skip"}"#,
        "\n",
    );
    let out = common::run_template_budget_no_color(
        input,
        "pseudo",
        10000,
        &["-i", "jsonl", "--grep", "needle"],
    );
    // Line 2 contains the match; its line number should appear in output
    assert!(
        out.contains("2:"),
        "grep output should preserve original JSONL line numbers; got: {out:?}"
    );
    assert!(out.contains("needle"), "matched value should be present");
}

// ---------------------------------------------------------------------------
// L. Fileset with JSONL files
// ---------------------------------------------------------------------------

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "Fileset JSONL integration test aggregates multiple assertions"
)]
fn jsonl_in_fileset_with_json() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let jsonl_path = dir.path().join("data.jsonl");
    let json_path = dir.path().join("config.json");
    std::fs::write(
        &jsonl_path,
        concat!(r#"{"line": 1}"#, "\n", r#"{"line": 2}"#, "\n",),
    )
    .unwrap();
    std::fs::write(&json_path, r#"{"key": "value"}"#).unwrap();

    let out = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "-c",
            "10000",
            "--no-sort",
            jsonl_path.file_name().unwrap().to_str().unwrap(),
            json_path.file_name().unwrap().to_str().unwrap(),
        ],
        None,
    );
    // Both files should appear in the fileset output
    assert!(
        out.stdout.contains("data.jsonl"),
        "JSONL file should appear in fileset output"
    );
    assert!(
        out.stdout.contains("config.json"),
        "JSON file should appear in fileset output"
    );
    // The JSONL data should be parsed (not treated as plain text)
    assert!(
        out.stdout.contains("line"),
        "JSONL content should be parsed as structured data in fileset"
    );
    // JSONL entries inside a fileset should show line numbers
    assert!(
        out.stdout.contains("1:"),
        "JSONL in fileset should show line number 1; got: {:?}",
        out.stdout
    );
    assert!(
        out.stdout.contains("2:"),
        "JSONL in fileset should show line number 2; got: {:?}",
        out.stdout
    );
}
