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

#[test]
fn weak_grep_conflicts_with_strong_grep() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--no-color",
            "--grep",
            "foo",
            "--weak-grep",
            "foo",
            "tests/fixtures/explicit/object_small.json",
        ])
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !ok,
        "cli should fail when --grep and --weak-grep are combined"
    );
    assert!(
        err.to_ascii_lowercase().contains("conflict")
            || err.to_ascii_lowercase().contains("cannot be used together")
            || err.to_ascii_lowercase().contains("cannot be used with"),
        "stderr should mention conflicting grep flags: {err}"
    );
}

#[test]
fn tree_conflicts_with_no_header() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--tree",
            "--no-header",
            "tests/fixtures/explicit/object_small.json",
        ])
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !ok,
        "cli should fail when --tree and --no-header are combined"
    );
    let err_l = err.to_ascii_lowercase();
    assert!(
        err_l.contains("cannot be used with") || err_l.contains("conflict"),
        "stderr should mention mutually exclusive flags: {err}"
    );
}

#[test]
fn tree_conflicts_with_compact() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--tree",
            "--compact",
            "tests/fixtures/explicit/object_small.json",
        ])
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    let err_l = err.to_ascii_lowercase();
    assert!(
        !ok,
        "cli should fail when --tree and --compact are combined"
    );
    assert!(
        err_l.contains("cannot be used with") || err_l.contains("conflict"),
        "stderr should mention tree/compact are incompatible: {err}"
    );
}

#[test]
fn tree_conflicts_with_no_newline() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--tree",
            "--no-newline",
            "tests/fixtures/explicit/object_small.json",
        ])
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    let err_l = err.to_ascii_lowercase();
    assert!(
        !ok,
        "cli should fail when --tree and --no-newline are combined"
    );
    assert!(
        err_l.contains("cannot be used with") || err_l.contains("conflict"),
        "stderr should mention tree/no-newline are incompatible: {err}"
    );
}

#[test]
fn tree_rejected_for_stdin() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd.args(["--tree"]).write_stdin("{}").assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !ok,
        "cli should fail when --tree is used without explicit file inputs (stdin mode)"
    );
    let err_l = err.to_ascii_lowercase();
    assert!(
        err_l.contains("tree")
            && (err_l.contains("stdin") || err_l.contains("input")),
        "stderr should mention tree mode requires file inputs, got: {err}"
    );
}

#[test]
fn grep_show_conflicts_with_weak_grep() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--no-color",
            "--weak-grep",
            "foo",
            "--grep-show",
            "all",
            "tests/fixtures/explicit/object_small.json",
        ])
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !ok,
        "cli should fail when --grep-show is used with --weak-grep"
    );
    let err_l = err.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict")
            || err_l.contains("cannot be used together")
            || err_l.contains("cannot be used with")
            || err_l.contains("requires"),
        "stderr should mention grep-show is incompatible with weak-grep: {err}"
    );
}
