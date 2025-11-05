use std::fs;

#[test]
fn debug_json_stdin() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "--debug",
            "-c",
            "120",
            "-f",
            "json",
            "-i",
            "json",
        ]) // explicit
        .write_stdin("{\"a\":1,\"b\":{\"c\":2}}\n")
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
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
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "--debug",
            "-c",
            "50",
            "-f",
            "text",
            "-i",
            "text",
        ]) // explicit
        .write_stdin("one\ntwo\nthree\n")
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
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

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "--debug", // capture stderr dump
            "-c",
            "10000",
            "-f",
            "auto",
            "-i",
            "yaml", // allow YAML ingest for fileset with yaml present
            p_json.to_str().unwrap(),
            p_yaml.to_str().unwrap(),
        ])
        .assert()
        .success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    let v: serde_json::Value =
        serde_json::from_str(&err).expect("stderr must be JSON");
    // format-agnostic debug dump; ensure structure present
    assert_eq!(v["root"]["fileset_root"], true);
    // Root metrics reflect total files present in the fileset
    assert_eq!(v["root"]["metrics"]["object_len"], 2);
}
