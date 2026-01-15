mod common;

#[test]
fn head_and_tail_flags_conflict() {
    // Pass both flags; clap should error with a conflict.
    let out = common::run_cli_expect_fail(
        &["--no-color", "--head", "--tail", "-n", "20", "-f", "json"], // no inputs (stdin not used)
        None,
        None,
    );
    assert!(
        out.stderr.to_ascii_lowercase().contains("conflict")
            || out.stderr.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {}",
        out.stderr
    );
}

#[test]
fn compact_and_no_newline_conflict() {
    // --compact conflicts with --no-newline via clap configuration.
    // Provide a small bytes budget to avoid other defaults interfering.
    let out = common::run_cli_expect_fail(
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
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {}",
        out.stderr
    );
}

#[test]
fn lines_and_no_newline_conflict() {
    // --no-newline conflicts with --lines
    let out = common::run_cli_expect_fail(
        &["--no-color", "--no-newline", "-n", "3", "-f", "json"],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {}",
        out.stderr
    );
}

#[test]
fn global_lines_and_no_newline_conflict() {
    // --no-newline conflicts with --global-lines
    let out = common::run_cli_expect_fail(
        &["--no-color", "--no-newline", "-N", "5", "-f", "json"],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {}",
        out.stderr
    );
}

#[test]
fn grep_show_requires_grep() {
    let out = common::run_cli_expect_fail(
        &[
            "--no-color",
            "--grep-show",
            "all",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("requires")
            || err_l.contains("missing")
            || err_l.contains("required arguments"),
        "stderr should mention missing --grep requirement: {}",
        out.stderr
    );
}

#[test]
fn weak_grep_conflicts_with_strong_grep() {
    let out = common::run_cli_expect_fail(
        &[
            "--no-color",
            "--grep",
            "foo",
            "--weak-grep",
            "foo",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    assert!(
        out.stderr.to_ascii_lowercase().contains("conflict")
            || out
                .stderr
                .to_ascii_lowercase()
                .contains("cannot be used together")
            || out
                .stderr
                .to_ascii_lowercase()
                .contains("cannot be used with"),
        "stderr should mention conflicting grep flags: {}",
        out.stderr
    );
}

#[test]
fn tree_conflicts_with_no_header() {
    let out = common::run_cli_expect_fail(
        &[
            "--tree",
            "--no-header",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("cannot be used with") || err_l.contains("conflict"),
        "stderr should mention mutually exclusive flags: {}",
        out.stderr
    );
}

#[test]
fn tree_conflicts_with_compact() {
    let out = common::run_cli_expect_fail(
        &[
            "--tree",
            "--compact",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("cannot be used with") || err_l.contains("conflict"),
        "stderr should mention tree/compact are incompatible: {}",
        out.stderr
    );
}

#[test]
fn tree_conflicts_with_no_newline() {
    let out = common::run_cli_expect_fail(
        &[
            "--tree",
            "--no-newline",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("cannot be used with") || err_l.contains("conflict"),
        "stderr should mention tree/no-newline are incompatible: {}",
        out.stderr
    );
}

#[test]
fn tree_rejected_for_stdin() {
    let out = common::run_cli_expect_fail(&["--tree"], Some(b"{}"), None);
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("tree")
            && (err_l.contains("stdin") || err_l.contains("input")),
        "stderr should mention tree mode requires file inputs, got: {}",
        out.stderr
    );
}

#[test]
fn grep_show_conflicts_with_weak_grep() {
    let out = common::run_cli_expect_fail(
        &[
            "--no-color",
            "--weak-grep",
            "foo",
            "--grep-show",
            "all",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict")
            || err_l.contains("cannot be used together")
            || err_l.contains("cannot be used with")
            || err_l.contains("requires"),
        "stderr should mention grep-show is incompatible with weak-grep: {}",
        out.stderr
    );
}

#[test]
fn recursive_conflicts_with_glob() {
    let out = common::run_cli_expect_fail(
        &["--recursive", "-g", "*.json"],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention recursive/glob conflict: {}",
        out.stderr
    );
}

#[test]
fn igrep_conflicts_with_grep() {
    let out = common::run_cli_expect_fail(
        &[
            "--no-color",
            "--grep",
            "foo",
            "--igrep",
            "bar",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention igrep/grep conflict: {}",
        out.stderr
    );
}

#[test]
fn igrep_conflicts_with_weak_grep() {
    let out = common::run_cli_expect_fail(
        &[
            "--no-color",
            "--igrep",
            "foo",
            "--weak-grep",
            "bar",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention igrep/weak-grep conflict: {}",
        out.stderr
    );
}

#[test]
fn igrep_conflicts_with_iweak_grep() {
    let out = common::run_cli_expect_fail(
        &[
            "--no-color",
            "--igrep",
            "foo",
            "--iweak-grep",
            "bar",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention igrep/iweak-grep conflict: {}",
        out.stderr
    );
}

#[test]
fn iweak_grep_conflicts_with_grep() {
    let out = common::run_cli_expect_fail(
        &[
            "--no-color",
            "--grep",
            "foo",
            "--iweak-grep",
            "bar",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention grep/iweak-grep conflict: {}",
        out.stderr
    );
}

#[test]
fn iweak_grep_conflicts_with_weak_grep() {
    let out = common::run_cli_expect_fail(
        &[
            "--no-color",
            "--weak-grep",
            "foo",
            "--iweak-grep",
            "bar",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict") || err_l.contains("cannot be used with"),
        "stderr should mention weak-grep/iweak-grep conflict: {}",
        out.stderr
    );
}

#[test]
fn grep_show_conflicts_with_iweak_grep() {
    let out = common::run_cli_expect_fail(
        &[
            "--no-color",
            "--iweak-grep",
            "foo",
            "--grep-show",
            "all",
            "tests/fixtures/explicit/object_small.json",
        ],
        None,
        None,
    );
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("conflict")
            || err_l.contains("cannot be used together")
            || err_l.contains("cannot be used with")
            || err_l.contains("requires"),
        "stderr should mention grep-show is incompatible with iweak-grep: {}",
        out.stderr
    );
}
