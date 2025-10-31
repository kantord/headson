use std::fs;

#[test]
fn single_file_auto_uses_json_ingest_and_output() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p = dir.path().join("data.json");
    fs::write(&p, b"{\n  \"a\": 1\n}\n").unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "-n",
            "10000",
            "-f",
            "auto",
            p.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(out.trim_start().starts_with('{'));
}
