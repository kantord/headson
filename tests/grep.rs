use assert_cmd::cargo::cargo_bin_cmd;
use headson::{
    Budgets, GrepConfig, InputKind, PriorityConfig, RenderConfig, Style,
};

// Covers strong --grep behavior (guaranteed inclusion path). Weak mode
// assertions belong in separate tests when implemented.

#[test]
fn grep_guarantees_match_even_when_budget_is_tiny() {
    let input = br#"{"outer":{"inner":"needle"},"other":"zzzz"}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--bytes",
            "5",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "needle",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("needle"),
        "match should be present even if it pushes past the user budget"
    );
    assert!(
        stdout.len() > 5,
        "effective budget should grow to fit the must-keep closure"
    );
}

#[test]
fn grep_keeps_ancestor_path_for_matches() {
    let input = br#"{"outer":{"inner":{"value":"match-me"}}}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-c",
            "8",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "match-me",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("match-me"),
        "matched leaf should always appear"
    );
    assert!(
        stdout.contains("outer") && stdout.contains("inner"),
        "ancestors of matched nodes should be kept so structure remains navigable"
    );
}

#[test]
fn grep_pins_sampled_array_elements() {
    let input = br#"[{"id":1},{"id":2},{"id":3,"value":"NEEDLE"},{"id":4,"value":"skip-me"}]"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--bytes",
            "12",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "NEEDLE",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("NEEDLE"),
        "array sampling should not drop matched elements in strong grep mode"
    );
    assert!(
        stdout.len() > 12,
        "strong grep should expand the effective budget beyond the user cap to include matches"
    );
}

#[test]
fn grep_highlights_matching_keys() {
    let input = br#"{"needle":123,"other":456}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-sort",
            "-c",
            "50",
            "-f",
            "json",
            "-t",
            "default",
            "--grep",
            "needle",
            "--no-header",
        ])
        .env("FORCE_COLOR", "1")
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\u{001b}[31mneedle\u{001b}[39m"),
        "matching keys should be highlighted when color is enabled"
    );
}

#[test]
fn grep_highlights_anchored_keys_without_quotes() {
    let input = br#"{"needle":123,"other":456}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-sort",
            "-c",
            "50",
            "-f",
            "json",
            "-t",
            "default",
            "--grep",
            "^needle$",
            "--no-header",
        ])
        .env("FORCE_COLOR", "1")
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\u{001b}[31mneedle\u{001b}[39m"),
        "anchored regex should highlight the matching key without requiring quotes; got: {stdout:?}"
    );
}

#[test]
fn grep_highlights_in_strict_style() {
    let input = br#"{"foo":"needle","bar":"other"}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--color",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "needle",
            "--no-sort",
            "--no-header",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\u{001b}[31mneedle\u{001b}[39m"),
        "grep should highlight matches even in strict style; got: {stdout:?}"
    );
    assert!(
        !stdout.contains("\u{001b}[1;34m") && !stdout.contains("\u{001b}[32m"),
        "only match highlights should be colored in strict grep mode; got: {stdout:?}"
    );
}

#[test]
fn grep_defaults_to_color_output() {
    let input = br#"{"k":"foo","x":"bar"}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args(["-f", "json", "-t", "default", "--grep", "foo", "--no-sort"])
        .env("FORCE_COLOR", "1")
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\u{001b}[31m"),
        "grep should emit colored matches by default; got: {stdout:?}"
    );
}

#[test]
fn grep_suppresses_syntax_colors_even_when_no_matches() {
    // With a grep pattern that matches nothing, syntax colors should still be off.
    let input = br#"{"a":1,"b":2}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "-f",
            "json",
            "-t",
            "default",
            "--grep",
            "nomatch",
            "--no-sort",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains("\u{001b}[32m") && !stdout.contains("\u{001b}[1;34m"),
        "syntax coloring should be disabled in grep mode even with zero matches; got: {stdout:?}"
    );
}

#[test]
fn grep_respects_auto_color_when_not_tty() {
    // Default (auto) color mode should avoid escape codes when stdout is not a TTY,
    // even if --grep is provided.
    let input = br#"{"needle": 1}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "-f",
            "json",
            "-t",
            "default",
            "--grep",
            "needle",
            "--no-sort",
            "--no-header",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains('\u{001b}'),
        "auto color should be disabled for non-TTY stdout; got escapes in: {stdout:?}"
    );
}

#[test]
fn grep_highlights_yaml_values_correctly() {
    let input = b"foo: bar\nmatch: baz\n".to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "-f",
            "yaml",
            "-i",
            "yaml",
            "-t",
            "default",
            "--grep",
            "baz",
            "--no-sort",
        ])
        .env("FORCE_COLOR", "1")
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\u{001b}[31mbaz\u{001b}[39m"),
        "expected exact match highlighting for YAML scalar values; got: {stdout:?}"
    );
}

#[test]
fn grep_does_not_highlight_json_punctuation() {
    let input = br#"{"a":1,"b":2}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args(["-f", "json", "-t", "default", "--grep", ":", "--no-sort"])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains("\u{001b}[31m:\u{001b}[39m"),
        "grep should not color structural punctuation: {stdout:?}"
    );
}

#[test]
fn grep_highlights_code_lines_without_syntax_colors() {
    // Small Rust-like snippet; grep should highlight only matches and not emit syntax colors.
    let input = b"fn build_order() {}\n// build something else\n".to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "-f",
            "text",
            "-i",
            "text",
            "-t",
            "default",
            "--grep",
            "build",
            "--no-sort",
            "--no-header",
        ])
        .env("FORCE_COLOR", "1")
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\u{001b}[31mbuild\u{001b}[39m"),
        "expected grep highlight in code-like text: {stdout:?}"
    );
    assert!(
        !stdout.contains("\u{001b}[32m") && !stdout.contains("\u{001b}[1;34m"),
        "syntax colors should be suppressed in grep mode for code/text: {stdout:?}"
    );
}

#[test]
fn grep_highlight_is_applied_once_per_value() {
    // Top-level string to exercise the direct leaf rendering path.
    let input = br#""foo""#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "-f",
            "json",
            "-t",
            "default",
            "--grep",
            "foo",
            "--bytes",
            "50",
            "--no-sort",
            "--no-header",
        ])
        .env("FORCE_COLOR", "1")
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\u{001b}[31mfoo\u{001b}[39m"),
        "expected single highlighted match in output; got: {stdout:?}"
    );
    assert!(
        !stdout.contains("\u{001b}[31m\u{001b}[31mfoo"),
        "matches should be highlighted once, without nested escapes; got: {stdout:?}"
    );
}

#[test]
fn grep_highlights_for_library_calls_without_extra_config() {
    let cfg = RenderConfig {
        template: headson::OutputTemplate::Pseudo,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: headson::ColorMode::On,
        color_enabled: true,
        style: Style::Default,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    };
    let prio = PriorityConfig::new(usize::MAX, usize::MAX);
    let budgets = Budgets {
        byte_budget: Some(200),
        char_budget: None,
        line_budget: None,
    };
    let grep = GrepConfig {
        regex: Some(regex::Regex::new("needle").unwrap()),
        weak: false,
    };
    let out = headson::headson(
        InputKind::Json(br#"{"needle":1,"other":2}"#.to_vec()),
        &cfg,
        &prio,
        &grep,
        budgets,
    )
    .expect("render");
    assert!(
        out.contains("\u{001b}[31mneedle\u{001b}[39m"),
        "library calls should auto-wire grep highlights when color is on: {out:?}"
    );
    // Syntax colors should be suppressed in grep mode.
    assert!(
        !out.contains("\u{001b}[1;34m") && !out.contains("\u{001b}[32m"),
        "grep mode should disable syntax colors for library calls: {out:?}"
    );
}
