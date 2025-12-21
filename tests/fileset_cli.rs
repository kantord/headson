mod common;
use std::fs;

#[test]
fn fileset_rejects_custom_format() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p_a = dir.path().join("a.txt");
    let p_b = dir.path().join("b.txt");
    fs::write(&p_a, "hello").expect("write a");
    fs::write(&p_b, "world").expect("write b");

    let out = common::run_cli_in_dir_expect_fail(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "-f",
            "text",
            "-c",
            "100",
            p_a.to_str().unwrap(),
            p_b.to_str().unwrap(),
        ],
        None,
        None,
    );
    let stderr = out.stderr;
    assert!(
        stderr.contains("--format cannot be customized for filesets"),
        "stderr missing rejection message: {stderr}"
    );
}
