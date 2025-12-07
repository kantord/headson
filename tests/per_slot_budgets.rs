use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use tempfile::{TempDir, tempdir};

fn write_file(dir: &TempDir, name: &str, contents: &str) {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdirs");
    }
    fs::write(path, contents).expect("write");
}

#[test]
fn per_file_line_budget_respected() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\na3\n");
    write_file(&dir, "b.txt", "b1\nb2\nb3\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "1", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("==> a.txt <==\na1\n"),
        "first file should keep only one line: {stdout}"
    );
    assert!(
        !stdout.contains("a2"),
        "first file should not exceed per-file line budget: {stdout}"
    );
    assert!(
        stdout.contains("==> b.txt <==\nb1\n"),
        "second file should still render under per-file line cap: {stdout}"
    );
    assert!(
        !stdout.contains("b2"),
        "second file should not exceed per-file line budget: {stdout}"
    );
}

#[test]
fn per_file_line_budget_counts_headers() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-H", "-n", "1", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("==> a.txt <=="),
        "header should render even when counted toward budget: {stdout}"
    );
    assert!(
        stdout.contains("==> b.txt <=="),
        "all slots should still appear under counted headers: {stdout}"
    );
    assert!(
        !stdout.contains("\na1\n") && !stdout.contains("\nb1\n"),
        "content should be skipped when header consumes the per-file line budget: {stdout}"
    );
}

#[test]
fn per_file_line_budget_zero_with_counted_headers_outputs_nothing() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-H", "-n", "0", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "per-file line budget of zero with counted headers should emit nothing: {stdout}"
    );
}

#[test]
fn per_file_line_budget_zero_without_headers_outputs_nothing() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "0", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "per-file line budget of zero without headers should emit nothing: {stdout}"
    );
}

#[test]
fn per_file_byte_budget_prevents_starvation() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "long.txt", "abcdefg\nhijklmn\n");
    write_file(&dir, "short.txt", "x\ny\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--bytes",
            "8",
            "long.txt",
            "short.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("==> long.txt <=="),
        "long file should still appear even when truncated by per-file bytes: {stdout}"
    );
    assert!(
        stdout.contains("==> short.txt <=="),
        "second file should not be starved by the first: {stdout}"
    );
    assert!(
        stdout.contains("x\n"),
        "short file should retain content under per-file byte cap: {stdout}"
    );
}

#[test]
fn per_file_line_budget_respected_with_strong_grep() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.py", "def f():\n    return 1\n");
    write_file(&dir, "b.py", "def g():\n    pass\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "return",
            "--grep-show",
            "all",
            "-n",
            "1",
            "a.py",
            "b.py",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("return 1"),
        "matching file should include the return line even under per-file cap: {stdout}"
    );
    assert!(
        stdout.contains("==> b.py <=="),
        "non-matching file should still render its header with --grep-show: {stdout}"
    );
    assert!(
        !stdout.contains("pass"),
        "non-matching tail content should stay filtered under the per-file cap: {stdout}"
    );
}

#[test]
fn per_file_line_budget_respected_without_headers() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--no-header",
            "-n",
            "1",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains("==>"),
        "headers should be suppressed with --no-header: {stdout}"
    );
    assert!(
        stdout.contains("a1"),
        "first file should still emit one line under per-file line budget: {stdout}"
    );
    assert!(
        stdout.contains("b1"),
        "second file should emit one line under per-file line budget: {stdout}"
    );
    assert!(
        !stdout.contains("a2") && !stdout.contains("b2"),
        "content beyond the per-file cap should be omitted: {stdout}"
    );
}

#[test]
fn per_file_grep_multiple_hits_are_not_dropped() {
    let dir = tempdir().expect("tmp");
    write_file(
        &dir,
        "match.py",
        "def f():\n    return 1\n    return 2\n    return 3\n    x = 1\n",
    );
    write_file(&dir, "other.py", "def g():\n    pass\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "return",
            "--grep-show",
            "all",
            "-n",
            "1",
            "match.py",
            "other.py",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.matches("return").count() >= 3,
        "all matching lines should be kept even when per-file cap is tight: {stdout}"
    );
    assert!(
        stdout.contains("==> other.py <=="),
        "non-matching file should still surface under --grep-show all: {stdout}"
    );
    assert!(
        !stdout.contains("pass"),
        "non-matching content should remain filtered under per-file cap: {stdout}"
    );
}

#[test]
fn per_file_byte_budget_counts_headers() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "aaaaaa\nbbbbbb\n");
    write_file(&dir, "b.txt", "cccccc\ndddddd\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "--bytes", "12", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("==> a.txt <=="),
        "header should consume from the per-file byte budget but still render: {stdout}"
    );
    assert!(
        stdout.contains("==> b.txt <=="),
        "second header should also render under the per-file byte cap: {stdout}"
    );
    assert!(
        !stdout.contains("bbbbbb") && !stdout.contains("dddddd"),
        "tails should be truncated once the per-file byte budget is hit: {stdout}"
    );
}

#[test]
fn global_line_budget_does_not_override_per_slot_cap() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\na3\na4\n");
    write_file(&dir, "b.txt", "b1\nb2\nb3\nb4\n");
    write_file(&dir, "c.txt", "c1\nc2\nc3\nc4\n");
    write_file(&dir, "d.txt", "d1\nd2\nd3\nd4\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "-n",
            "2",
            "--global-lines",
            "50",
            "a.txt",
            "b.txt",
            "c.txt",
            "d.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for prefix in ["a", "b", "c", "d"] {
        assert!(
            stdout.contains(&format!("{prefix}1")),
            "each file should keep at least the first line under per-file cap: {stdout}"
        );
        assert!(
            !stdout.contains(&format!("{prefix}2")),
            "second line should be trimmed when omission marker must fit under the per-file cap: {stdout}"
        );
        assert!(
            stdout.contains("…"),
            "truncation marker should still appear while respecting per-file cap: {stdout}"
        );
    }
}

#[test]
fn per_file_line_budget_does_not_drop_small_files() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "5", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("a1\na2"),
        "small file should render fully when under per-file line budget: {stdout}"
    );
    assert!(
        stdout.contains("b1\nb2"),
        "second small file should also render fully: {stdout}"
    );
    assert!(
        !stdout.contains("…"),
        "no omission markers expected when under budget: {stdout}"
    );
}

#[test]
fn per_file_line_budget_keeps_string_prefix_in_line_only_mode() {
    let dir = tempdir().expect("tmp");
    write_file(
        &dir,
        "a.json",
        &format!("{{\"long\":\"{}\"}}", "A".repeat(400)),
    );
    write_file(
        &dir,
        "b.json",
        &format!("{{\"long\":\"{}\"}}", "B".repeat(400)),
    );

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "1", "a.json", "b.json"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("AAAAA"),
        "line-only per-file cap should still show a string prefix: {stdout}"
    );
    assert!(
        stdout.contains("BBBBB"),
        "line-only per-file cap should show a prefix for each slot: {stdout}"
    );
}
