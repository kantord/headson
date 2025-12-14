use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use tempfile::tempdir;

#[test]
fn per_slot_and_global_zero_caps_emit_nothing() {
    let dir = tempdir().expect("tmp");
    fs::write(dir.path().join("a.txt"), "a1\na2\n").unwrap();
    fs::write(dir.path().join("b.txt"), "b1\nb2\n").unwrap();

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "-n",
            "0",
            "--global-lines",
            "0",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "combined zero per-file and global caps should suppress output: {stdout:?}"
    );
}

#[test]
fn tree_header_budgeting_differs_when_headers_are_charged() {
    let dir = tempdir().expect("tmp");
    fs::write(dir.path().join("a.txt"), "a1\na2\na3\n").unwrap();
    fs::write(dir.path().join("b.txt"), "b1\nb2\nb3\n").unwrap();

    let default = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--tree",
            "-n",
            "2",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();
    let counted = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--tree",
            "-H",
            "-n",
            "2",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();

    let default_out = String::from_utf8_lossy(&default.get_output().stdout);
    let counted_out = String::from_utf8_lossy(&counted.get_output().stdout);
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
fn section_headers_charged_under_line_caps() {
    let dir = tempdir().expect("tmp");
    fs::write(dir.path().join("a.txt"), "a1\na2\na3\n").unwrap();
    fs::write(dir.path().join("b.txt"), "b1\nb2\nb3\n").unwrap();

    let free = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "2", "a.txt", "b.txt"])
        .assert()
        .success();
    let charged = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-H", "-n", "2", "a.txt", "b.txt"])
        .assert()
        .success();

    let free_out = String::from_utf8_lossy(&free.get_output().stdout);
    let charged_out = String::from_utf8_lossy(&charged.get_output().stdout);
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

    let strong = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "needle",
            "--global-lines",
            "0",
            "only.txt",
        ])
        .assert()
        .success();
    let weak = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--weak-grep",
            "needle",
            "--global-lines",
            "0",
            "only.txt",
        ])
        .assert()
        .success();

    let strong_out = String::from_utf8_lossy(&strong.get_output().stdout);
    let weak_out = String::from_utf8_lossy(&weak.get_output().stdout);
    assert!(
        strong_out.contains("needle") && !strong_out.trim().is_empty(),
        "must-keep matches should still render even when the global budget is zero: {strong_out}"
    );
    assert!(
        weak_out.trim().is_empty(),
        "weak grep should obey the zero global budget and emit nothing: {weak_out:?}"
    );
}
