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
