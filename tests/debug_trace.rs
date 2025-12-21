mod common;
use std::fs;

#[test]
fn debug_json_stdin() {
    let output = common::run_cli(
        &[
            "--no-color",
            "--debug",
            "-c",
            "120",
            "-f",
            "json",
            "-i",
            "json",
        ], // explicit
        Some("{\"a\":1,\"b\":{\"c\":2}}\n".as_bytes()),
    );
    let out = output.stdout;
    let err = output.stderr;
    assert!(!out.trim().is_empty(), "stdout must not be empty");
    let v: serde_json::Value =
        serde_json::from_str(&err).expect("stderr must be JSON");
    // format-agnostic debug dump; ensure structure present
    assert!(v["counts"]["included"].as_u64().unwrap_or(0) >= 1);
    // Root should be object
    assert_eq!(v["root"]["kind"], "object");
}

#[test]
fn debug_text_stdin() {
    let output = common::run_cli(
        &[
            "--no-color",
            "--debug",
            "-c",
            "50",
            "-f",
            "text",
            "-i",
            "text",
        ], // explicit
        Some("one\ntwo\nthree\n".as_bytes()),
    );
    let out = output.stdout;
    let err = output.stderr;
    assert!(!out.trim().is_empty(), "stdout must not be empty");
    let v: serde_json::Value =
        serde_json::from_str(&err).expect("stderr must be JSON");
    // format-agnostic debug dump; ensure structure present
    assert!(v["counts"]["included"].as_u64().unwrap_or(0) >= 1);
}

#[test]
fn debug_fileset_two_inputs() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p_json = dir.path().join("a.json");
    let p_yaml = dir.path().join("b.yaml");
    fs::write(&p_json, b"{\n  \"a\": 1\n}\n").unwrap();
    fs::write(&p_yaml, b"k: 2\n").unwrap();

    let output = common::run_cli(
        &[
            "--no-color",
            "--debug", // capture stderr dump
            "--no-sort",
            "-c",
            "10000",
            "-f",
            "auto",
            "-i",
            "yaml", // allow YAML ingest for fileset with yaml present
            p_json.to_str().unwrap(),
            p_yaml.to_str().unwrap(),
        ],
        None,
    );
    let err = output.stderr;
    let v: serde_json::Value =
        serde_json::from_str(&err).expect("stderr must be JSON");
    // format-agnostic debug dump; ensure structure present
    assert_eq!(v["root"]["fileset_root"], true);
    // Root metrics reflect total files present in the fileset
    assert_eq!(v["root"]["metrics"]["object_len"], 2);
}
