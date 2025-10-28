use assert_cmd::Command;

#[test]
fn color_and_no_color_flags_conflict() {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .args(["--color", "--no-color", "-n", "10", "-f", "json"]) // no input; parse-only
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(!ok, "cli should fail on color flag conflict");
    assert!(
        err.to_ascii_lowercase().contains("cannot be used with")
            || err.to_ascii_lowercase().contains("conflict"),
        "stderr should mention conflict, got: {err}"
    );
}

#[test]
fn color_and_no_color_flags_parse_and_run() {
    // Provide minimal JSON via stdin so the command runs.
    let input = b"{}";
    for flag in ["--color", "--no-color"] {
        let mut cmd = Command::cargo_bin("headson").expect("bin");
        let assert = cmd
            .args([flag, "-n", "10", "-f", "json"]) // simple json output
            .write_stdin(input.as_slice())
            .assert()
            .success();
        let out = String::from_utf8_lossy(&assert.get_output().stdout);
        assert!(!out.trim().is_empty());
    }
}
