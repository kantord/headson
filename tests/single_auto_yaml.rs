mod common;
use std::fs;

#[test]
fn single_file_auto_uses_yaml_ingest_and_output() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p = dir.path().join("data.yaml");
    fs::write(&p, b"k: 2\n").unwrap();

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
    let out = out.stdout;
    assert!(out.contains("k:"), "expected YAML key in output: {out:?}");
}
