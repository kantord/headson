use assert_cmd::Command;

fn run_stdin(args: &[&str], input: &str) -> String {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let out = cmd.args(args).write_stdin(input).output().expect("run");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn e2e_grep_weak_prefers_value_match() {
    // Baseline favors alpha-first key; grep-weak should bias to the value match.
    let input = r#"{ "aaa": "foo", "zzz": "llibre" }"#;
    let base = run_stdin(&["-n", "30", "-f", "js", "--compact"], input);
    assert!(base.contains("\"aaa\":"), "baseline alpha-first: {base}");
    assert!(!base.contains("\"zzz\":"), "baseline omits zzz: {base}");

    let biased = run_stdin(
        &["-n", "30", "-f", "js", "--compact", "--grep-weak", "llibre"],
        input,
    );
    assert!(
        biased.contains("\"zzz\":"),
        "grep-weak should include zzz branch: {biased}"
    );
    assert!(
        !biased.contains("\"aaa\":"),
        "tight budget should keep only one key: {biased}"
    );
}

#[test]
fn e2e_grep_weak_prefers_descendant_match() {
    let input = r#"{ "aaa": {"k": "foo"}, "zzz": {"k": "llibre"} }"#;
    let base = run_stdin(&["-n", "30", "-f", "js", "--compact"], input);
    assert!(base.contains("\"aaa\":"), "baseline alpha-first: {base}");
    assert!(!base.contains("\"zzz\":"), "baseline omits zzz: {base}");

    let biased = run_stdin(
        &["-n", "30", "-f", "js", "--compact", "--grep-weak", "llibre"],
        input,
    );
    assert!(
        biased.contains("\"zzz\":"),
        "grep-weak should include path with descendant match: {biased}"
    );
    assert!(
        !biased.contains("\"aaa\":"),
        "tight budget should keep only one key: {biased}"
    );
}

#[test]
fn e2e_grep_weak_key_name_match() {
    let input = r#"{ "libre_item": 1, "aaaa": 2 }"#;
    let base = run_stdin(&["-n", "24", "-f", "js", "--compact"], input);
    assert!(base.contains("\"aaaa\":"), "baseline alpha-first: {base}");
    assert!(
        !base.contains("\"libre_item\":"),
        "baseline omits libre: {base}"
    );

    let biased = run_stdin(
        &["-n", "24", "-f", "js", "--compact", "--grep-weak", "libre"],
        input,
    );
    assert!(
        biased.contains("\"libre_item\":"),
        "grep-weak should bias key name match: {biased}"
    );
    assert!(
        !biased.contains("\"aaaa\":"),
        "tight budget should keep only one key: {biased}"
    );
}
