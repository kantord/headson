mod common;

#[test]
fn color_and_no_color_flags_conflict() {
    let out = common::run_cli(
        &["--color", "--no-color", "-c", "10", "-f", "json"], // no input; parse-only
        None,
    );
    assert!(!out.ok, "cli should fail on color flag conflict");
    assert!(
        out.stderr
            .to_ascii_lowercase()
            .contains("cannot be used with")
            || out.stderr.to_ascii_lowercase().contains("conflict"),
        "stderr should mention conflict, got: {}",
        out.stderr
    );
}

#[test]
fn color_and_no_color_flags_parse_and_run() {
    // Provide minimal JSON via stdin so the command runs.
    let input = b"{}";
    for flag in ["--color", "--no-color"] {
        let out =
            common::run_cli(&[flag, "-c", "10", "-f", "json"], Some(input));
        assert!(out.ok, "cli should succeed for flag {flag}");
        assert!(!out.stdout.trim().is_empty());
    }
}
