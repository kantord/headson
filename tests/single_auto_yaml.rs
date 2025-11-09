use std::fs;

#[test]
fn single_file_auto_uses_yaml_ingest_and_output() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p = dir.path().join("data.yaml");
    fs::write(&p, b"k: 2\n").unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--no-color",
            "-c",
            "10000",
            "-f",
            "auto",
            p.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(out.contains("k:"), "expected YAML key in output: {out:?}");
}
