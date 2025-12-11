use assert_cmd::cargo::cargo_bin_cmd;
use headson::{
    Budgets, GrepConfig, InputKind, PriorityConfig, RenderConfig, Style,
};
use tempfile::tempdir;

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
fn grep_counts_matches_as_free_for_char_budgets() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("sample.txt");
    std::fs::write(
        &path,
        "this line has a match keyword and is long\nshort\n",
    )
    .unwrap();
    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--chars",
            "15",
            "--grep",
            "keyword",
            path.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("short"),
        "non-matching context should still render when matches are free under char budgets; got: {stdout:?}"
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
fn grep_filters_out_files_without_matches_in_filesets() {
    let dir = tempdir().unwrap();
    let with = dir.path().join("with.json");
    let without = dir.path().join("without.json");
    std::fs::write(&with, br#"{"keep":"needle"}"#).unwrap();
    std::fs::write(&without, br#"{"drop":0}"#).unwrap();

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--grep",
            "needle",
            "--no-sort",
            with.file_name().unwrap().to_str().unwrap(),
            without.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("needle"));
    assert!(
        !stdout.contains("without.json"),
        "files without matches should be filtered out of fileset renders",
    );
    assert!(
        !stdout.contains("more files"),
        "filtered files should not be counted in fileset summaries",
    );
}

#[test]
fn grep_show_all_keeps_non_matching_files_in_filesets() {
    let dir = tempdir().unwrap();
    let with = dir.path().join("with.json");
    let without = dir.path().join("without.json");
    std::fs::write(&with, br#"{"keep":"needle"}"#).unwrap();
    std::fs::write(&without, br#"{"drop":0}"#).unwrap();

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--grep",
            "needle",
            "--grep-show",
            "all",
            "--no-sort",
            with.file_name().unwrap().to_str().unwrap(),
            without.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("needle"),
        "matching content should still be present with --grep-show=all"
    );
    assert!(
        stdout.contains("without.json"),
        "non-matching files should render when --grep-show=all"
    );
}

#[test]
fn grep_filtered_files_produce_identical_output() {
    // Two invocations with identical settings: one only includes matching files,
    // the other adds extra files with no matches. Outputs must be byte-for-byte equal.
    let dir = tempdir().unwrap();
    let with_a = dir.path().join("with_a.json");
    let with_b = dir.path().join("with_b.json");
    let without = dir.path().join("without.json");
    std::fs::write(&with_a, br#"{"keep":"needle","other":1}"#).unwrap();
    std::fs::write(&with_b, br#"{"keep":"needle","more":2}"#).unwrap();
    std::fs::write(&without, br#"{"drop":0}"#).unwrap();

    let base = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--grep",
            "needle",
            "--bytes",
            "40",
            "--no-sort",
            with_a.file_name().unwrap().to_str().unwrap(),
            with_b.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let with_extra = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--grep",
            "needle",
            "--bytes",
            "40",
            "--no-sort",
            with_a.file_name().unwrap().to_str().unwrap(),
            with_b.file_name().unwrap().to_str().unwrap(),
            without.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let base_out = base.get_output().stdout.clone();
    let extra_out = with_extra.get_output().stdout.clone();
    assert_eq!(
        base_out, extra_out,
        "adding files without matches should not change grep output",
    );
}

#[test]
fn grep_ignores_filename_only_matches_in_filesets() {
    let dir = tempdir().unwrap();
    let matching = dir.path().join("foo.json");
    let filename_match = dir.path().join("needle_only.json");
    std::fs::write(&matching, br#"{ "keep": "needle" }"#).unwrap();
    // No content matches; only the filename contains the pattern.
    std::fs::write(&filename_match, br#"{ "drop": 0 }"#).unwrap();

    let base = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--grep",
            "needle",
            "--bytes",
            "80",
            "--no-sort",
            matching.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let with_filename_match = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--grep",
            "needle",
            "--bytes",
            "80",
            "--no-sort",
            matching.file_name().unwrap().to_str().unwrap(),
            filename_match.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let base_body = String::from_utf8_lossy(&base.get_output().stdout);
    let with_body =
        String::from_utf8_lossy(&with_filename_match.get_output().stdout);
    // Fileset runs should keep only the content matches and not count filename-only
    // matches as hits. Headers may differ, but the rendered payload should match.
    let strip_header = |s: &str| {
        let mut lines = s.lines();
        if let Some(first) = lines.next() {
            if first.starts_with("==> ") && first.ends_with(" <==") {
                return lines.collect::<Vec<_>>().join("\n");
            }
        }
        s.to_string()
    };
    assert_eq!(
        strip_header(&base_body).trim_end(),
        strip_header(&with_body).trim_end(),
        "filenames matching the pattern should not force files into grep output",
    );
    assert!(
        !with_body.contains("needle_only.json"),
        "filename-only matches should not be rendered or counted"
    );
}

#[test]
fn grep_show_matching_matches_default_behavior() {
    let dir = tempdir().unwrap();
    let with = dir.path().join("with.json");
    let without = dir.path().join("without.json");
    std::fs::write(&with, br#"{"keep":"needle"}"#).unwrap();
    std::fs::write(&without, br#"{"drop":0}"#).unwrap();

    let default = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--grep",
            "needle",
            "--no-sort",
            with.file_name().unwrap().to_str().unwrap(),
            without.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let explicit = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--grep",
            "needle",
            "--grep-show",
            "matching",
            "--no-sort",
            with.file_name().unwrap().to_str().unwrap(),
            without.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(
        default.get_output().stdout,
        explicit.get_output().stdout,
        "--grep-show=matching should mirror the default grep fileset filtering"
    );
}

#[test]
fn grep_fileset_without_matches_renders_nothing() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.json");
    let b = dir.path().join("b.json");
    std::fs::write(&a, br#"{"foo":1}"#).unwrap();
    std::fs::write(&b, br#"[1,2,3]"#).unwrap();

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "NEEDLE",
            a.file_name().unwrap().to_str().unwrap(),
            b.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "filesets with zero grep matches should render nothing, got: {stdout:?}"
    );
}

#[test]
fn grep_fileset_without_matches_emits_notice() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.json");
    let b = dir.path().join("b.json");
    std::fs::write(&a, br#"{"foo":1}"#).unwrap();
    std::fs::write(&b, br#"[1,2,3]"#).unwrap();

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "NEEDLE",
            a.file_name().unwrap().to_str().unwrap(),
            b.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("No grep matches found"),
        "expected a notice about missing grep matches for fileset run: {stderr:?}"
    );
}

#[test]
fn grep_does_not_shrink_global_budget_when_filtering_filesets() {
    // Adding a file with no matches should not reduce the effective global budget.
    let dir = tempdir().unwrap();
    let with_a = dir.path().join("with_a.json");
    let with_b = dir.path().join("with_b.json");
    let without = dir.path().join("without.json");
    let payload = "A".repeat(400);
    std::fs::write(&with_a, format!(r#"{{"keep":"hit","big":"{payload}"}}"#))
        .unwrap();
    std::fs::write(&with_b, format!(r#"{{"keep":"hit","big":"{payload}"}}"#))
        .unwrap();
    std::fs::write(&without, br#"{"drop":0}"#).unwrap();

    let base = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--no-header",
            "--string-cap",
            "2000",
            "--global-bytes",
            "900",
            "--grep",
            "hit",
            with_a.file_name().unwrap().to_str().unwrap(),
            with_b.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let with_extra = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--no-header",
            "--string-cap",
            "2000",
            "--global-bytes",
            "900",
            "--grep",
            "hit",
            with_a.file_name().unwrap().to_str().unwrap(),
            with_b.file_name().unwrap().to_str().unwrap(),
            without.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let base_out = base.get_output().stdout.clone();
    let extra_out = with_extra.get_output().stdout.clone();
    assert_eq!(
        base_out, extra_out,
        "adding non-matching files should not shrink the effective global budget"
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
        fileset_tree: false,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    };
    let prio = PriorityConfig::new(usize::MAX, usize::MAX);
    let budgets = Budgets {
        global: Some(headson::Budget {
            kind: headson::BudgetKind::Bytes,
            cap: 200,
        }),
        per_slot: None,
    };
    let grep = GrepConfig {
        regex: Some(regex::Regex::new("needle").unwrap()),
        weak: false,
        show: headson::GrepShow::Matching,
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

#[test]
fn weak_grep_does_not_expand_budget_or_guarantee_match() {
    let input = br#"{"keep":"needle"}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--bytes",
            "5",
            "-f",
            "json",
            "-t",
            "strict",
            "--weak-grep",
            "needle",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.len() <= 5,
        "weak grep should not expand the user-provided byte budget; got len {}",
        stdout.len()
    );
}

#[test]
fn weak_grep_keeps_non_matching_files_in_filesets() {
    let dir = tempdir().unwrap();
    let matching = dir.path().join("with.json");
    let other = dir.path().join("without.json");
    std::fs::write(&matching, br#"{"keep":"needle"}"#).unwrap();
    std::fs::write(&other, br#"{"drop":0}"#).unwrap();

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--weak-grep",
            "needle",
            "--no-sort",
            matching.file_name().unwrap().to_str().unwrap(),
            other.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("needle"),
        "matching content should still render with weak grep"
    );
    assert!(
        stdout.contains("without.json"),
        "weak grep should not filter out files without matches"
    );
}

#[test]
fn weak_grep_highlights_matches_without_syntax_colors() {
    let input = br#"{"k":"needle","x":"other"}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "-f",
            "json",
            "-t",
            "default",
            "--weak-grep",
            "needle",
            "--no-sort",
            "--no-header",
        ])
        .env("FORCE_COLOR", "1")
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\u{001b}[31mneedle\u{001b}[39m"),
        "weak grep should still highlight matches when color is enabled"
    );
    assert!(
        !stdout.contains("\u{001b}[1;34m") && !stdout.contains("\u{001b}[32m"),
        "syntax colors should be suppressed in weak grep mode: {stdout:?}"
    );
}

#[test]
fn weak_grep_biases_sampling_toward_matches() {
    // Object order: non-matching field first, then the match. With a tiny budget,
    // weak grep should bias priority so the matched field is the one that survives.
    let input = br#"{"miss":"xxxxxxxxxx","hit":"needle"}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--bytes",
            "20",
            "-f",
            "json",
            "-t",
            "strict",
            "--weak-grep",
            "needle",
            "--no-sort",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\"hit\""),
        "weak grep should bias sampling so the matched field is kept under tight budgets: {stdout:?}"
    );
    assert!(
        !stdout.contains("\"miss\""),
        "non-matching fields should be more likely to be pruned first in weak grep mode: {stdout:?}"
    );
    assert!(
        stdout.len() <= 20,
        "weak grep must still respect the user-provided budget"
    );
}

#[test]
fn weak_grep_fileset_with_no_matches_still_renders_and_has_no_notice() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.json");
    let b = dir.path().join("b.json");
    std::fs::write(&a, br#"{"foo":1}"#).unwrap();
    std::fs::write(&b, br#"{"bar":2}"#).unwrap();

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--weak-grep",
            "NEEDLE",
            "--no-sort",
            a.file_name().unwrap().to_str().unwrap(),
            b.file_name().unwrap().to_str().unwrap(),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !stdout.trim().is_empty(),
        "weak grep should not drop all fileset content when no matches are found"
    );
    assert!(
        !stderr.contains("No grep matches found"),
        "weak grep should not emit the strong-grep notice when there are no matches"
    );
}

#[test]
fn strong_grep_obeys_zero_global_line_budget_for_non_matches() {
    let input = br#"{"keep":"needle","drop":"filler"}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "needle",
            "--grep-show",
            "all",
            "--global-lines",
            "0",
        ])
        .write_stdin(input)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("needle"),
        "strong grep should still surface matching content when budgets are zeroed: {stdout}"
    );
    assert!(
        !stdout.contains("drop"),
        "non-matching content should be excluded when no global line headroom remains: {stdout}"
    );
}
