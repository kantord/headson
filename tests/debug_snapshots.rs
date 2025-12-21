mod common;
use std::fs;

#[test]
fn snapshot_debug_json_stdin_strict_combined() {
    let output = common::run_cli(
        &[
            "--no-color",
            "--debug",
            "-c",
            "200",
            "-f",
            "json",
            "-t",
            "strict",
            "-i",
            "json",
        ], // strict -> template "json"
        Some("{\"a\":1,\"b\":{\"c\":2}}\n".as_bytes()),
    );
    let out = output.stdout;
    let err = output.stderr;
    let norm = common::normalize_debug(&err);
    let snap = format!("STDOUT:\n{out}\nDEBUG (normalized):\n{norm}\n");
    insta::assert_snapshot!("debug_json_stdin_strict_combined", snap);
}

#[test]
fn snapshot_debug_fileset_auto_combined() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p_json = dir.path().join("a.json");
    let p_yaml = dir.path().join("b.yaml");
    fs::write(&p_json, b"{\n  \"a\": 1\n}\n").unwrap();
    fs::write(&p_yaml, b"k: 2\n").unwrap();

    let output = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--debug",
            "-c",
            "20000",
            "-f",
            "auto",
            "-i",
            "yaml",
            "a.json",
            "b.yaml",
        ],
        None,
    );
    let out = output.stdout;
    let err = output.stderr;
    let norm = common::normalize_debug(&err);
    let snap = format!("STDOUT:\n{out}\nDEBUG (normalized):\n{norm}\n");
    insta::assert_snapshot!("debug_fileset_auto_combined", snap);
}
