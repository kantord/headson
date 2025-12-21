mod common;
use std::fs;
use tempfile::tempdir;

#[test]
fn per_slot_and_global_zero_caps_emit_nothing() {
    let dir = tempdir().expect("tmp");
    fs::write(dir.path().join("a.txt"), "a1\na2\n").unwrap();
    fs::write(dir.path().join("b.txt"), "b1\nb2\n").unwrap();

    let out = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "-n",
            "0",
            "--global-lines",
            "0",
            "a.txt",
            "b.txt",
        ],
        None,
    );
    let stdout = out.stdout;
    assert!(
        stdout.trim().is_empty(),
        "combined zero per-file and global caps should suppress output: {stdout:?}"
    );
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single test compares multiple output variants for clarity"
)]
fn tree_header_budgeting_differs_when_headers_are_charged() {
    let dir = tempdir().expect("tmp");
    fs::write(dir.path().join("a.txt"), "a1\na2\na3\n").unwrap();
    fs::write(dir.path().join("b.txt"), "b1\nb2\nb3\n").unwrap();

    let default = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "--tree",
            "-n",
            "2",
            "a.txt",
            "b.txt",
        ],
        None,
    );
    let counted = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "--tree",
            "-H",
            "-n",
            "2",
            "a.txt",
            "b.txt",
        ],
        None,
    );

    let default_out = default.stdout;
    let counted_out = counted.stdout;
    assert!(
        default_out.contains("a1") && default_out.contains("b1"),
        "tree render should surface body lines when headers are free: {default_out}"
    );
    assert!(
        counted_out.contains("… 2 more items"),
        "charging headers should push tree mode under the cap: {counted_out}"
    );
    assert!(
        !counted_out.contains("a1") && !counted_out.contains("b1"),
        "charged header budgeting should elide body lines first: {counted_out}"
    );
    assert_ne!(
        default_out, counted_out,
        "tree output should differ once header budgeting is charged"
    );
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single test compares multiple output variants for clarity"
)]
fn section_headers_charged_under_line_caps() {
    let dir = tempdir().expect("tmp");
    fs::write(dir.path().join("a.txt"), "a1\na2\na3\n").unwrap();
    fs::write(dir.path().join("b.txt"), "b1\nb2\nb3\n").unwrap();

    let free = common::run_cli_in_dir(
        dir.path(),
        &["--no-color", "--no-sort", "-n", "2", "a.txt", "b.txt"],
        None,
    );
    let charged = common::run_cli_in_dir(
        dir.path(),
        &["--no-color", "--no-sort", "-H", "-n", "2", "a.txt", "b.txt"],
        None,
    );

    let free_out = free.stdout;
    let charged_out = charged.stdout;
    assert!(
        free_out.contains("a1") && free_out.contains("b1"),
        "section mode should still surface content when headers are free: {free_out}"
    );
    assert!(
        charged_out.contains("==> 2 more files <=="),
        "charged headers should consume the cap and emit a summary: {charged_out}"
    );
    assert!(
        !charged_out.contains("a1") && !charged_out.contains("b1"),
        "section bodies should be trimmed once headers are charged: {charged_out}"
    );
}

#[test]
fn strong_vs_weak_grep_under_zero_global_lines() {
    let dir = tempdir().expect("tmp");
    fs::write(dir.path().join("only.txt"), "alpha\nneedle\nomega\n").unwrap();

    let strong = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "--grep",
            "needle",
            "--global-lines",
            "0",
            "only.txt",
        ],
        None,
    );
    let weak = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "--weak-grep",
            "needle",
            "--global-lines",
            "0",
            "only.txt",
        ],
        None,
    );

    let strong_out = strong.stdout;
    let weak_out = weak.stdout;
    assert!(
        strong_out.contains("needle") && !strong_out.trim().is_empty(),
        "must-keep matches should still render even when the global budget is zero: {strong_out}"
    );
    assert!(
        weak_out.trim().is_empty(),
        "weak grep should obey the zero global budget and emit nothing: {weak_out:?}"
    );
}
