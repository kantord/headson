mod common;

fn run_ok(args: &[&str], stdin: Option<&[u8]>) -> common::CliOutput {
    common::run_cli(args, stdin)
}

#[test]
fn multiple_grep_flags_match_either_pattern() {
    let input = br#"{"a":"foo","b":"bar","c":"other"}"#.to_vec();
    let out = run_ok(
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
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("foo") && stdout.contains("bar"),
        "multiple --grep flags should match either pattern; got: {stdout:?}"
    );
}

#[test]
fn multiple_igrep_flags_match_either_pattern() {
    let input = br#"{"a":"FOO","b":"BAR","c":"other"}"#.to_vec();
    let out = run_ok(
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
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("FOO") && stdout.contains("BAR"),
        "multiple --igrep flags should match either pattern case-insensitively; got: {stdout:?}"
    );
}

#[test]
fn mixed_grep_and_igrep_flags_combine() {
    let input = br#"{"a":"foo","b":"BAR","c":"other"}"#.to_vec();
    let out = run_ok(
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
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("foo") && stdout.contains("BAR"),
        "--grep and --igrep should combine: foo (case-sensitive) + bar (case-insensitive); got: {stdout:?}"
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
fn multiple_iweak_grep_flags_bias_toward_either_pattern() {
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
            "--iweak-grep",
            "foo",
            "--iweak-grep",
            "bar",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("FOO") || stdout.contains("BAR"),
        "multiple --iweak-grep flags should bias toward either match case-insensitively; got: {stdout:?}"
    );
}

#[test]
fn mixed_weak_grep_and_iweak_grep_flags_combine() {
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
            "--iweak-grep",
            "bar",
        ],
        Some(&input),
    );
    let stdout = out.stdout;
    assert!(
        stdout.contains("foo") || stdout.contains("BAR"),
        "--weak-grep and --iweak-grep should combine; got: {stdout:?}"
    );
}
