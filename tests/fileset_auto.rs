mod common;
use std::fs;

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single test validates multiple file outputs in one flow"
)]
fn auto_template_uses_per_file_format_in_fileset() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p_json = dir.path().join("a.json");
    let p_yaml = dir.path().join("b.yaml");
    fs::write(&p_json, b"{\n  \"a\": 1\n}\n").unwrap();
    fs::write(&p_yaml, b"k: 2\n").unwrap();

    let out = common::run_cli(
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "10000",
            "-f",
            "auto",
            "-i",
            "yaml",
            p_json.to_str().unwrap(),
            p_yaml.to_str().unwrap(),
        ],
        None,
    );
    assert!(out.status.success(), "cli should succeed");
    let out = String::from_utf8_lossy(&out.stdout);
    // Should contain both headers, and JSON/YAML style bodies respectively.
    assert!(out.contains("a.json"));
    assert!(out.contains("b.yaml"));
    let after_json = out.split("a.json").nth(1).unwrap();
    assert!(after_json.contains('{'));
    let after_yaml = out.split("b.yaml").nth(1).unwrap();
    assert!(after_yaml.contains("k:"));
}
