mod common;

#[test]
fn color_and_no_color_flags_conflict() {
    let out = common::run_cli(
        &["--color", "--no-color", "-c", "10", "-f", "json"], // no input; parse-only
        None,
    );
    assert!(
        !out.status.success(),
        "cli should fail on color flag conflict"
    );
    let err = String::from_utf8_lossy(&out.stderr);
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
        let out =
            common::run_cli(&[flag, "-c", "10", "-f", "json"], Some(input));
        assert!(out.status.success(), "cli should succeed for flag {flag}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(!stdout.trim().is_empty());
    }
}
