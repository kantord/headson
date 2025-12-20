mod common;
use std::fs;

#[test]
fn fileset_auto_uses_yaml_ingest_when_uppercase_yaml_present() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p_json = dir.path().join("a.json");
    let p_yaml = dir.path().join("B.YML");
    fs::write(&p_json, b"{\n  \"a\": 1\n}\n").unwrap();
    fs::write(&p_yaml, b"k: 2\n").unwrap();

    // No -i yaml; rely on Auto ingest selection
    let out = common::run_cli(
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "10000",
            "-f",
            "auto",
            p_json.to_str().unwrap(),
            p_yaml.to_str().unwrap(),
        ],
        None,
    );
    assert!(out.status.success(), "cli should succeed");
    let out = String::from_utf8_lossy(&out.stdout);
    // Expect both headers and respective body styles.
    assert!(out.contains("a.json"));
    assert!(out.contains("B.YML"));
    // After the YAML header, the section should contain a YAML key
    let after_yaml = out.split("B.YML").nth(1).unwrap_or("");
    assert!(after_yaml.contains("k:"));
}
