mod common;

use std::ffi::OsStr;

/// Test to verify regex flag leak bug: (?i) in one pattern should not affect other patterns
#[test]
fn inline_regex_flags_should_not_leak_between_patterns() {
    // When using --grep '(?i)foo' --grep 'bar', the (?i) from first pattern
    // should NOT make 'bar' case-insensitive.
    // BAR should be truncated (not a match) since only "bar" lowercase matches.
    assert_grep_output(
        &[
            "--no-color",
            "--no-sort",
            "-f",
            "json",
            "-t",
            "default",
            "--bytes",
            "10", // tight budget forces truncation of non-matches
            "--grep",
            "(?i)foo",
            "--grep",
            "bar",
        ],
        br#"{"a":"FOO","b":"BAR","c":"bar"}"#,
        &["FOO", "bar"], // FOO matches (?i)foo, bar matches case-sensitive bar
        &["BAR"],        // BAR should be truncated - if present, (?i) leaked
    );
}

fn run_ok(args: &[&str], stdin: Option<&[u8]>) -> common::CliOutput {
    common::run_cli(args, stdin)
}

fn run_ok_color(args: &[&str], stdin: Option<&[u8]>) -> common::CliOutput {
    let envs = [("FORCE_COLOR", OsStr::new("1"))];
    common::run_cli_in_dir_env(".", args, stdin, &envs)
}

/// Test helper to reduce boilerplate in grep tests.
/// Runs CLI with given args and input, then asserts output contains/excludes expected strings.
fn assert_grep_output(
    args: &[&str],
    input: &[u8],
    must_contain: &[&str],
    must_not_contain: &[&str],
) {
    let out = run_ok(args, Some(input));
    let stdout = &out.stdout;
    for expected in must_contain {
        assert!(
            stdout.contains(expected),
            "expected {expected:?} in output; got: {stdout:?}"
        );
    }
    for unexpected in must_not_contain {
        assert!(
            !stdout.contains(unexpected),
            "unexpected {unexpected:?} in output; got: {stdout:?}"
        );
    }
}

#[test]
fn multiple_grep_flags_match_either_pattern() {
    assert_grep_output(
        &[
            "--no-color",
            "--no-sort",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "foo",
            "--grep",
            "bar",
        ],
        br#"{"a":"foo","b":"bar","c":"other"}"#,
        &["foo", "bar"],
        &[],
    );
}

#[test]
fn multiple_igrep_flags_match_either_pattern() {
    assert_grep_output(
        &[
            "--no-color",
            "--no-sort",
            "-f",
            "json",
            "-t",
            "strict",
            "--igrep",
            "foo",
            "--igrep",
            "bar",
        ],
        br#"{"a":"FOO","b":"BAR","c":"other"}"#,
        &["FOO", "BAR"],
        &[],
    );
}

#[test]
fn mixed_grep_and_igrep_flags_combine() {
    assert_grep_output(
        &[
            "--no-color",
            "--no-sort",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "foo",
            "--igrep",
            "bar",
        ],
        br#"{"a":"foo","b":"BAR","c":"other"}"#,
        &["foo", "BAR"],
        &[],
    );
}

#[test]
fn multiple_weak_grep_flags_bias_toward_either_pattern() {
    let input = br#"{"a":"foo","b":"bar","c":"xxxxxxxxxxxxx"}"#.to_vec();
    let out = run_ok(
        &[
            "--no-color",
            "--no-sort",
            "--bytes",
            "60",
            "-f",
            "json",
            "-t",
            "strict",
            "--weak-grep",
            "foo",
            "--weak-grep",
            "bar",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("foo") || stdout.contains("bar"),
        "multiple --weak-grep flags should bias toward either match; got: {stdout:?}"
    );
}

#[test]
fn multiple_weak_igrep_flags_bias_toward_either_pattern() {
    let input = br#"{"a":"FOO","b":"BAR","c":"xxxxxxxxxxxxx"}"#.to_vec();
    let out = run_ok(
        &[
            "--no-color",
            "--no-sort",
            "--bytes",
            "60",
            "-f",
            "json",
            "-t",
            "strict",
            "--weak-igrep",
            "foo",
            "--weak-igrep",
            "bar",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("FOO") || stdout.contains("BAR"),
        "multiple --weak-igrep flags should bias toward either match case-insensitively; got: {stdout:?}"
    );
}

#[test]
fn mixed_weak_grep_and_weak_igrep_flags_combine() {
    let input = br#"{"a":"foo","b":"BAR","c":"xxxxxxxxxxxxx"}"#.to_vec();
    let out = run_ok(
        &[
            "--no-color",
            "--no-sort",
            "--bytes",
            "60",
            "-f",
            "json",
            "-t",
            "strict",
            "--weak-grep",
            "foo",
            "--weak-igrep",
            "bar",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("foo") || stdout.contains("BAR"),
        "--weak-grep and --weak-igrep should combine; got: {stdout:?}"
    );
}

#[test]
fn grep_and_weak_grep_can_be_combined() {
    // "must" matches strong --grep (guaranteed), "bias" matches --weak-grep (priority)
    let input = br#"{"a":"must","b":"bias","c":"other"}"#.to_vec();
    let out = run_ok(
        &[
            "--no-color",
            "--no-sort",
            "--bytes",
            "60",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "must",
            "--weak-grep",
            "bias",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("must"),
        "--grep match should be guaranteed; got: {stdout:?}"
    );
}

#[test]
fn grep_and_weak_igrep_can_be_combined() {
    let input = br#"{"a":"must","b":"BIAS","c":"other"}"#.to_vec();
    let out = run_ok(
        &[
            "--no-color",
            "--no-sort",
            "--bytes",
            "60",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "must",
            "--weak-igrep",
            "bias",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("must"),
        "--grep match should be guaranteed; got: {stdout:?}"
    );
}

#[test]
fn igrep_and_weak_grep_can_be_combined() {
    let input = br#"{"a":"MUST","b":"bias","c":"other"}"#.to_vec();
    let out = run_ok(
        &[
            "--no-color",
            "--no-sort",
            "--bytes",
            "60",
            "-f",
            "json",
            "-t",
            "strict",
            "--igrep",
            "must",
            "--weak-grep",
            "bias",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("MUST"),
        "--igrep match should be guaranteed; got: {stdout:?}"
    );
}

#[test]
fn igrep_and_weak_igrep_can_be_combined() {
    let input = br#"{"a":"MUST","b":"BIAS","c":"other"}"#.to_vec();
    let out = run_ok(
        &[
            "--no-color",
            "--no-sort",
            "--bytes",
            "60",
            "-f",
            "json",
            "-t",
            "strict",
            "--igrep",
            "must",
            "--weak-igrep",
            "bias",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("MUST"),
        "--igrep match should be guaranteed; got: {stdout:?}"
    );
}

#[test]
fn all_four_grep_flags_can_be_combined() {
    let input =
        br#"{"a":"must","b":"IMUST","c":"bias","d":"IBIAS","e":"other"}"#
            .to_vec();
    let out = run_ok(
        &[
            "--no-color",
            "--no-sort",
            "--bytes",
            "100",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "must",
            "--igrep",
            "imust",
            "--weak-grep",
            "bias",
            "--weak-igrep",
            "ibias",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("must") && stdout.contains("IMUST"),
        "strong grep matches should be guaranteed; got: {stdout:?}"
    );
}

#[test]
fn weak_grep_biases_priority_when_combined_with_strong() {
    // Array with: guaranteed match, weak match, and filler
    // With tight budget, weak match should be prioritized over filler
    let input =
        br#"["guaranteed","weak_match","filler_aa","filler_bb"]"#.to_vec();
    let out = run_ok(
        &[
            "--no-color",
            "--no-sort",
            "--bytes",
            "55",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "guaranteed",
            "--weak-grep",
            "weak_match",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("guaranteed"),
        "strong match should be guaranteed; got: {stdout:?}"
    );
    assert!(
        stdout.contains("weak_match"),
        "weak match should be prioritized over non-matching filler; got: {stdout:?}"
    );
}

#[test]
fn both_strong_and_weak_patterns_are_highlighted() {
    let input = br#"{"a":"strong_val","b":"weak_val"}"#.to_vec();
    let out = run_ok_color(
        &[
            "-f",
            "json",
            "-t",
            "default",
            "--grep",
            "strong",
            "--weak-grep",
            "weak",
            "--no-sort",
        ],
        Some(&input),
    );
    let stdout = out.stdout_ansi;
    // Check that both "strong" and "weak" are highlighted (wrapped in ANSI red)
    assert!(
        stdout.contains("\x1b[31mstrong\x1b[39m"),
        "strong pattern should be highlighted; got: {stdout:?}"
    );
    assert!(
        stdout.contains("\x1b[31mweak\x1b[39m"),
        "weak pattern should be highlighted; got: {stdout:?}"
    );
}

#[test]
fn fileset_with_strong_and_weak_grep_filters_on_strong_matches_only() {
    // File with strong match, file with only weak match, file with neither.
    // With --grep-show=matching (default), only the file with strong matches should appear.
    let dir = tempfile::tempdir().unwrap();
    let strong_file = dir.path().join("strong.json");
    let weak_only_file = dir.path().join("weak_only.json");
    let neither_file = dir.path().join("neither.json");

    std::fs::write(&strong_file, br#"{"a":"needle"}"#).unwrap();
    std::fs::write(&weak_only_file, br#"{"a":"bias"}"#).unwrap();
    std::fs::write(&neither_file, br#"{"a":"other"}"#).unwrap();

    let out = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "--grep",
            "needle",
            "--weak-grep",
            "bias",
            "strong.json",
            "weak_only.json",
            "neither.json",
        ],
        None,
    );
    let stdout = out.stdout;

    assert!(
        stdout.contains("needle"),
        "file with strong match should be included; got: {stdout:?}"
    );
    assert!(
        !stdout.contains("weak_only.json"),
        "file with only weak match should be filtered out by --grep-show=matching; got: {stdout:?}"
    );
    assert!(
        !stdout.contains("neither.json"),
        "file with no matches should be filtered out; got: {stdout:?}"
    );
}

#[test]
fn fileset_with_strong_and_weak_grep_show_all_includes_weak_only_files() {
    // With --grep-show=all, files with only weak matches should still appear.
    let dir = tempfile::tempdir().unwrap();
    let strong_file = dir.path().join("strong.json");
    let weak_only_file = dir.path().join("weak_only.json");

    std::fs::write(&strong_file, br#"{"a":"needle"}"#).unwrap();
    std::fs::write(&weak_only_file, br#"{"a":"bias"}"#).unwrap();

    let out = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "--grep",
            "needle",
            "--weak-grep",
            "bias",
            "--grep-show",
            "all",
            "strong.json",
            "weak_only.json",
        ],
        None,
    );
    let stdout = out.stdout;

    assert!(
        stdout.contains("needle"),
        "file with strong match should be included; got: {stdout:?}"
    );
    assert!(
        stdout.contains("weak_only.json"),
        "file with only weak match should be included with --grep-show=all; got: {stdout:?}"
    );
    assert!(
        stdout.contains("bias"),
        "weak match content should be present with --grep-show=all; got: {stdout:?}"
    );
}

#[test]
fn fileset_with_no_strong_matches_but_weak_matches_renders_nothing() {
    // When --grep pattern matches nothing but --weak-grep matches something,
    // the fileset should render nothing (strong grep controls filtering).
    let dir = tempfile::tempdir().unwrap();
    let weak_only = dir.path().join("weak_only.json");
    let other = dir.path().join("other.json");

    std::fs::write(&weak_only, br#"{"a":"bias"}"#).unwrap();
    std::fs::write(&other, br#"{"a":"other"}"#).unwrap();

    let out = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "--grep",
            "nonexistent",
            "--weak-grep",
            "bias",
            "weak_only.json",
            "other.json",
        ],
        None,
    );

    assert!(
        out.stdout.trim().is_empty(),
        "fileset with no strong matches should render nothing even if weak matches exist; got: {:?}",
        out.stdout
    );
    assert!(
        out.stderr.contains("No grep matches found"),
        "should emit notice about no grep matches; got: {:?}",
        out.stderr
    );
}

#[test]
fn fileset_tree_with_only_weak_matches_is_empty_and_warns() {
    // In tree mode, --grep-show=matching still filters by strong matches only.
    // Weak-only matches should not make files appear.
    let dir = tempfile::tempdir().unwrap();
    let weak_only = dir.path().join("weak_only.json");
    let other = dir.path().join("other.json");

    std::fs::write(&weak_only, br#"{"a":"bias"}"#).unwrap();
    std::fs::write(&other, br#"{"a":"other"}"#).unwrap();

    let out = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "--tree",
            "--grep",
            "nonexistent",
            "--weak-grep",
            "bias",
            "weak_only.json",
            "other.json",
        ],
        None,
    );

    assert!(
        out.stdout.trim().is_empty(),
        "tree fileset should be empty when no strong matches exist (weak matches do not include files); got: {:?}",
        out.stdout
    );
    assert!(
        out.stderr.contains("No grep matches found"),
        "tree fileset should warn when no strong matches exist; got: {:?}",
        out.stderr
    );
}
