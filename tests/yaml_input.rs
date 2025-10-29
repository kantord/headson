#[path = "../test_support/mod.rs"]
mod util;
use assert_cmd::Command;
use insta::assert_snapshot;
use std::fs;

#[test]
fn stdin_yaml_basic_end_to_end() {
    // Basic mapping + sequence
    let y = "foo:\n  - list1\n  - 2\nbar: true\n";
    // Render in YAML template as well, but ingestion is YAML via -i yaml
    let out = util::run_template_budget(y, "yaml", 10_000, &["-i", "yaml"]);
    // Expect top-level YAML mapping keys present
    assert!(
        out.contains("foo:"),
        "expected key 'foo' in output: {out:?}"
    );
    assert!(
        out.contains("bar:"),
        "expected key 'bar' in output: {out:?}"
    );
}

#[test]
fn stdin_yaml_basic_snapshot() {
    let y = "foo:\n  - list1\n  - 2\nbar: true\n";
    let out = util::run_template_budget(y, "yaml", 10_000, &["-i", "yaml"]);
    assert_snapshot!("yaml_stdin_basic", out);
}

#[test]
fn file_yaml_basic_end_to_end() {
    let tmp = tempfile::tempdir().expect("tmp");
    let p = tmp.path().join("data.yaml");
    fs::write(&p, b"a: 1\narr: [x, y, z]\n").expect("write yaml");
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .args([
            "--no-color",
            "-n",
            "10000",
            "-f",
            "yaml",
            "-i",
            "yaml",
            p.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(out.contains("a:"));
    assert!(out.contains("arr:"));
}
