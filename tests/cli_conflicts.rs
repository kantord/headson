#[test]
fn head_and_tail_flags_conflict() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    // Pass both flags; clap should error with a conflict.
    let assert = cmd
        .args(["--no-color", "--head", "--tail", "-n", "20", "-f", "json"]) // no inputs (stdin not used)
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(!ok, "cli should fail when both --head and --tail are set");
    assert!(
        err.to_ascii_lowercase().contains("conflict")
            || err.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {err}"
    );
}

#[test]
fn compact_and_no_newline_conflict() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    // --compact conflicts with --no-newline via clap configuration.
    // Provide a small bytes budget to avoid other defaults interfering.
    let assert = cmd
        .args([
            "--no-color",
            "--compact",
            "--no-newline",
            "-c",
            "100",
            "-f",
            "json",
        ])
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !ok,
        "cli should fail when both --compact and --no-newline are set",
    );
    let err_l = err.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {err}"
    );
}

#[test]
fn lines_and_no_newline_conflict() {
    // --no-newline conflicts with --lines
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args(["--no-color", "--no-newline", "-n", "3", "-f", "json"])
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !ok,
        "cli should fail when both --no-newline and --lines are set",
    );
    let err_l = err.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {err}"
    );
}

#[test]
fn global_lines_and_no_newline_conflict() {
    // --no-newline conflicts with --global-lines
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args(["--no-color", "--no-newline", "-N", "5", "-f", "json"])
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !ok,
        "cli should fail when both --no-newline and --global-lines are set",
    );
    let err_l = err.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {err}"
    );
}

#[test]
fn grep_show_requires_grep() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--no-color",
            "--grep-show",
            "all",
            "tests/fixtures/explicit/object_small.json",
        ])
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !ok,
        "cli should fail when --grep-show is used without --grep"
    );
    let err_l = err.to_ascii_lowercase();
    assert!(
        err_l.contains("requires")
            || err_l.contains("missing")
            || err_l.contains("required arguments"),
        "stderr should mention missing --grep requirement: {err}"
    );
}
