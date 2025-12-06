use assert_cmd::cargo::cargo_bin_cmd;
use insta::assert_snapshot;

fn normalize(out: &str) -> String {
    out.replace('\\', "/")
}

#[test]
fn snapshot_grep_per_slot_line_cap() {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "return",
            "--grep-show",
            "all",
            "-n",
            "1",
            "tests/fixtures/code/sample.py",
            "tests/fixtures/code/sample.ts",
        ])
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_snapshot!("grep_per_slot_line_cap", normalize(&out));
}

#[test]
fn snapshot_counted_headers_tiny_line_cap() {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "-H",
            "-n",
            "1",
            "tests/fixtures/mixed_headers/a.json",
            "tests/fixtures/mixed_headers/b.yaml",
            "tests/fixtures/mixed_headers/c.txt",
        ])
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_snapshot!("counted_headers_tiny_line_cap", normalize(&out));
}

#[test]
fn snapshot_tree_per_slot_line_cap() {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--tree",
            "--glob",
            "tests/fixtures/tree_per_slot/*.txt",
            "-n",
            "1",
        ])
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_snapshot!("tree_per_slot_line_cap", normalize(&out));
}

#[test]
fn snapshot_tree_per_slot_varied_line_cap() {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--tree",
            "--glob",
            "tests/fixtures/tree_per_slot_varied/*.txt",
            "-n",
            "3",
        ])
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_snapshot!("tree_per_slot_varied_line_cap", normalize(&out));
}

#[test]
fn snapshot_multibyte_chars_and_bytes_per_slot() {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "--bytes",
            "6",
            "--chars",
            "6",
            "tests/fixtures/bytes_chars/emoji.json",
            "tests/fixtures/bytes_chars/long.txt",
        ])
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_snapshot!("multibyte_chars_and_bytes_per_slot", normalize(&out));
}

#[test]
fn snapshot_multibyte_chars_tighter_than_bytes() {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "--tree",
            "--chars",
            "12",
            "--bytes",
            "100",
            "tests/fixtures/chars_vs_bytes/emoji.txt",
            "tests/fixtures/chars_vs_bytes/ascii.txt",
        ])
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_snapshot!("multibyte_chars_tighter_than_bytes", normalize(&out));
}
