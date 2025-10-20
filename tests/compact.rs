use assert_cmd::Command;

fn run(input: &str, extra: &[&str]) -> assert_cmd::assert::Assert {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    cmd.args(["-n", "1000", "-f", "json"]).args(extra).write_stdin(input).assert()
}

#[test]
fn compact_minifies_output() {
    let input = r#"{"a": [1, 2, 3], "b": {"c": 1, "d": 2}}"#;
    let assert = run(input, &["--compact"]).success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let trimmed = stdout.trim_end_matches('\n').to_string();
    assert!(!trimmed.contains('\n'), "no newlines in compact output");
    assert!(!trimmed.contains("  "), "no double spaces from indent");
    assert!(!trimmed.contains(": "), "no space after colon");
    // Basic shape check
    assert_eq!(trimmed, "{\"a\":[1,2,3],\"b\":{\"c\":1,\"d\":2}}".to_string());
}

#[test]
fn compact_conflicts_with_other_flags() {
    let input = r#"{"a":1}"#;
    // --compact with --no-newline should error (clap conflict)
    run(input, &["--compact", "--no-newline"]).failure();
    // --compact with --no-space should error
    run(input, &["--compact", "--no-space"]).failure();
    // --compact with --indent should error
    run(input, &["--compact", "--indent", "\t"]).failure();
}

