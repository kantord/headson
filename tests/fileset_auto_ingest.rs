use std::fs;

#[test]
fn auto_mode_picks_yaml_ingest_for_mixed_files() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p_json = dir.path().join("a.json");
    let p_yaml = dir.path().join("b.yaml");
    fs::write(&p_json, b"{\n  \"a\": 1\n}\n").unwrap();
    fs::write(&p_yaml, b"k: 2\n").unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    // Do not pass -i yaml; rely on Auto ingest selection for fileset
    let assert = cmd
        .args([
            "--no-color",
            "-n",
            "10000",
            "-f",
            "auto",
            p_json.to_str().unwrap(),
            p_yaml.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(out.contains("a.json") && out.contains("b.yaml"));
}
