use assert_cmd::Command;

fn run_with_flags(input: &str, template: &str, extra: &[&str]) -> String {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let mut args = vec!["-n", "1000", "-f", template];
    args.extend_from_slice(extra);
    let output = cmd.args(args).write_stdin(input).output().expect("run");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn no_newline_flag_makes_single_line() {
    // Non-trivial JSON that normally renders with newlines
    let input = r#"{"a": [1, 2, 3], "b": {"c": 1, "d": 2}}"#;
    let templates = ["json", "pseudo", "js"];

    for tmpl in templates {
        let multi = run_with_flags(input, tmpl, &[]);
        let multi_trimmed = multi.trim_end_matches('\n');
        assert!(
            multi_trimmed.contains('\n'),
            "expected multi-line output for {tmpl}"
        );

        let single = run_with_flags(input, tmpl, &["--no-newline"]);
        let single_trimmed = single.trim_end_matches('\n');
        assert!(
            !single_trimmed.contains('\n'),
            "expected single-line output for {tmpl}, got: {single:?}"
        );

        if tmpl == "json" {
            serde_json::from_str::<serde_json::Value>(&multi)
                .expect("json (multi) should parse");
            serde_json::from_str::<serde_json::Value>(&single)
                .expect("json (single) should parse");
        }
    }
}
