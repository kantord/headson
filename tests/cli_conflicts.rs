mod common;

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn head_and_tail_flags_conflict() {
    // Pass both flags; clap should error with a conflict.
    let out = common::run_cli(
        &["--no-color", "--head", "--tail", "-n", "20", "-f", "json"], // no inputs (stdin not used)
        None,
    );
    assert!(
        !out.status.success(),
        "cli should fail when both --head and --tail are set"
    );
    assert!(
        stderr(&out).to_ascii_lowercase().contains("conflict")
            || stderr(&out).contains("cannot be used with"),
        "stderr should mention argument conflict, got: {}",
        stderr(&out)
    );
}

#[test]
fn compact_and_no_newline_conflict() {
    // --compact conflicts with --no-newline via clap configuration.
    // Provide a small bytes budget to avoid other defaults interfering.
    let out = common::run_cli(
        &[
            "--no-color",
            "--compact",
            "--no-newline",
            "-c",
            "100",
            "-f",
            "json",
        ],
        None,
    );
    assert!(
        !out.status.success(),
        "cli should fail when both --compact and --no-newline are set",
    );
    let err_l = stderr(&out).to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {}",
        stderr(&out)
    );
}

#[test]
fn lines_and_no_newline_conflict() {
    // --no-newline conflicts with --lines
    let out = common::run_cli(
        &["--no-color", "--no-newline", "-n", "3", "-f", "json"],
        None,
    );
    assert!(
        !out.status.success(),
        "cli should fail when both --no-newline and --lines are set",
    );
    let err_l = stderr(&out).to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {}",
        stderr(&out)
    );
}

#[test]
fn global_lines_and_no_newline_conflict() {
    // --no-newline conflicts with --global-lines
    let out = common::run_cli(
        &["--no-color", "--no-newline", "-N", "5", "-f", "json"],
        None,
    );
    assert!(
        !out.status.success(),
        "cli should fail when both --no-newline and --global-lines are set",
    );
    let err_l = stderr(&out).to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {}",
        stderr(&out)
    );
}

#[test]
fn grep_show_requires_grep() {
    let out = common::run_cli(
        &[
            "--no-color",
            "--grep-show",
            "all",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
    );
    assert!(
        !out.status.success(),
        "cli should fail when --grep-show is used without --grep"
    );
    let err_l = stderr(&out).to_ascii_lowercase();
    assert!(
        err_l.contains("requires")
            || err_l.contains("missing")
            || err_l.contains("required arguments"),
        "stderr should mention missing --grep requirement: {}",
        stderr(&out)
    );
}

#[test]
fn weak_grep_conflicts_with_strong_grep() {
    let out = common::run_cli(
        &[
            "--no-color",
            "--grep",
            "foo",
            "--weak-grep",
            "foo",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
    );
    assert!(
        !out.status.success(),
        "cli should fail when --grep and --weak-grep are combined"
    );
    assert!(
        stderr(&out).to_ascii_lowercase().contains("conflict")
            || stderr(&out)
                .to_ascii_lowercase()
                .contains("cannot be used together")
            || stderr(&out)
                .to_ascii_lowercase()
                .contains("cannot be used with"),
        "stderr should mention conflicting grep flags: {}",
        stderr(&out)
    );
}

#[test]
fn tree_conflicts_with_no_header() {
    let out = common::run_cli(
        &[
            "--tree",
            "--no-header",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
    );
    assert!(
        !out.status.success(),
        "cli should fail when --tree and --no-header are combined"
    );
    let err_l = stderr(&out).to_ascii_lowercase();
    assert!(
        err_l.contains("cannot be used with") || err_l.contains("conflict"),
        "stderr should mention mutually exclusive flags: {}",
        stderr(&out)
    );
}

#[test]
fn tree_conflicts_with_compact() {
    let out = common::run_cli(
        &[
            "--tree",
            "--compact",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
    );
    let err_l = stderr(&out).to_ascii_lowercase();
    assert!(
        !out.status.success(),
        "cli should fail when --tree and --compact are combined"
    );
    assert!(
        err_l.contains("cannot be used with") || err_l.contains("conflict"),
        "stderr should mention tree/compact are incompatible: {}",
        stderr(&out)
    );
}

#[test]
fn tree_conflicts_with_no_newline() {
    let out = common::run_cli(
        &[
            "--tree",
            "--no-newline",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
    );
    let err_l = stderr(&out).to_ascii_lowercase();
    assert!(
        !out.status.success(),
        "cli should fail when --tree and --no-newline are combined"
    );
    assert!(
        err_l.contains("cannot be used with") || err_l.contains("conflict"),
        "stderr should mention tree/no-newline are incompatible: {}",
        stderr(&out)
    );
}

#[test]
fn tree_rejected_for_stdin() {
    let out = common::run_cli(&["--tree"], Some(b"{}"));
    assert!(
        !out.status.success(),
        "cli should fail when --tree is used without explicit file inputs (stdin mode)"
    );
    let err_l = stderr(&out).to_ascii_lowercase();
    assert!(
        err_l.contains("tree")
            && (err_l.contains("stdin") || err_l.contains("input")),
        "stderr should mention tree mode requires file inputs, got: {}",
        stderr(&out)
    );
}

#[test]
fn grep_show_conflicts_with_weak_grep() {
    let out = common::run_cli(
        &[
            "--no-color",
            "--weak-grep",
            "foo",
            "--grep-show",
            "all",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
    );
    assert!(
        !out.status.success(),
        "cli should fail when --grep-show is used with --weak-grep"
    );
    let err_l = stderr(&out).to_ascii_lowercase();
    assert!(
        err_l.contains("conflict")
            || err_l.contains("cannot be used together")
            || err_l.contains("cannot be used with")
            || err_l.contains("requires"),
        "stderr should mention grep-show is incompatible with weak-grep: {}",
        stderr(&out)
    );
}
