mod common;
use std::fs;

#[test]
fn single_file_auto_uses_json_ingest_and_output() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p = dir.path().join("data.json");
    fs::write(&p, b"{\n  \"a\": 1\n}\n").unwrap();

    let out = common::run_cli(
        &[
            "--no-color",
            "-c",
            "10000",
            "-f",
            "auto",
            p.to_str().unwrap(),
        ],
        None,
    );
    assert!(out.status.success(), "cli should succeed");
    let out = String::from_utf8_lossy(&out.stdout);
    assert!(out.trim_start().starts_with('{'));
}
