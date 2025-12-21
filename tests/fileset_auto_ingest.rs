mod common;
use std::fs;

#[test]
fn auto_mode_picks_yaml_ingest_for_mixed_files() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p_json = dir.path().join("a.json");
    let p_yaml = dir.path().join("b.yaml");
    fs::write(&p_json, b"{\n  \"a\": 1\n}\n").unwrap();
    fs::write(&p_yaml, b"k: 2\n").unwrap();

    // Do not pass -i yaml; rely on Auto ingest selection for fileset
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
    let out = out.stdout;
    assert!(out.contains("a.json") && out.contains("b.yaml"));
}
