use std::fs;

fn temp_file(contents: &str) -> tempfile::NamedTempFile {
    let f = tempfile::NamedTempFile::new().expect("tempfile");
    fs::write(f.path(), contents).expect("write tmp file");
    f
}

#[test]
fn rejects_conflicting_per_file_metrics() {
    let file = temp_file("hello");
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "-n",
            "5",
            "-u",
            "100",
            file.path().to_str().unwrap(),
        ])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("only one per-file budget"),
        "stderr should mention conflicting per-file metrics: {stderr}"
    );
}

#[test]
fn rejects_conflicting_global_metrics() {
    let file = temp_file("hello");
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "-C",
            "100",
            "-N",
            "5",
            file.path().to_str().unwrap(),
        ])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("only one global budget"),
        "stderr should mention conflicting global metrics: {stderr}"
    );
}

#[test]
fn allows_mixed_levels() {
    let file = temp_file("line one\nline two\nline three");
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "-n",
            "2",
            "-C",
            "80",
            file.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.lines().filter(|l| !l.is_empty()).count() <= 2,
        "per-file line cap should hold even with global bytes: {stdout:?}"
    );
}

#[test]
fn global_bytes_too_small_yields_empty() {
    let file = temp_file(r#"{"a":1}"#);
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "-C",
            "1",
            file.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "outputs should be empty when the global byte budget cannot fit any content: {stdout:?}"
    );
}

#[test]
fn global_lines_zero_yields_empty() {
    let file = temp_file("hello\nworld\n");
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "-N",
            "0",
            file.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "outputs should be empty when the global line budget is zero: {stdout:?}"
    );
}

#[test]
fn per_file_byte_budget_one_renders_nothing() {
    let path = "tests/fixtures/bytes_chars/emoji.json";
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args(["--no-color", "--no-sort", "--bytes", "1", path])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "when a 1-byte per-file/global cap leaves no room for content, output should be empty: {stdout:?}"
    );
}
