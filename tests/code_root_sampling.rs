use assert_cmd::cargo::cargo_bin_cmd;
use insta::assert_snapshot;

fn run_sample_py_auto() -> String {
    let assert = cargo_bin_cmd!("headson")
        .args([
            "--no-color",
            "-c",
            "120",
            "-f",
            "auto",
            "tests/fixtures/code/sample.py",
        ])
        .assert()
        .success();
    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

#[test]
fn code_auto_sample_snapshot() {
    let out = run_sample_py_auto();
    assert_snapshot!("code_auto_sample_snapshot", out);
}

#[test]
fn code_prefers_top_level_headers() {
    let assert = cargo_bin_cmd!("headson")
        .args([
            "--no-color",
            "-c",
            "120",
            "-f",
            "auto",
            "tests/fixtures/code/sample.py",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("def main"),
        "expected top-level def main to appear:\n{out}"
    );
    assert!(
        out.contains("def compute"),
        "expected top-level def compute to appear:\n{out}"
    );
}
