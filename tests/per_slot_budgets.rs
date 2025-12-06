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
