#[test]
fn head_and_tail_flags_conflict() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
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
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
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
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
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
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
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
fn chars_and_bytes_conflict() {
    // --chars conflicts with --bytes
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args(["--no-color", "-u", "100", "-c", "100", "-f", "json"])
        .assert();
    assert!(!assert.get_output().status.success());
}

#[test]
fn chars_and_global_bytes_conflict() {
    // --chars conflicts with --global-bytes
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args(["--no-color", "-u", "100", "-C", "100", "-f", "json"])
        .assert();
    assert!(!assert.get_output().status.success());
}

#[test]
fn global_chars_and_bytes_conflict() {
    // --global-chars conflicts with --bytes
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args(["--no-color", "-U", "100", "-c", "100", "-f", "json"])
        .assert();
    assert!(!assert.get_output().status.success());
}

#[test]
fn global_chars_and_global_bytes_conflict() {
    // --global-chars conflicts with --global-bytes
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args(["--no-color", "-U", "100", "-C", "100", "-f", "json"])
        .assert();
    assert!(!assert.get_output().status.success());
}
