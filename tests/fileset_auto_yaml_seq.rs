use std::fs;

#[test]
fn auto_fileset_renders_yaml_sequence_of_mappings_properly() {
    // Use real fixtures to ensure YAML arrays-of-objects render with dash items
    // and YAML key syntax, not JSON braces.
    let p_json = "tests/fixtures/explicit/object_small.json";
    let p_yaml = "tests/fixtures/yaml/yaml-test-suite/229Q.yaml";

    // Ensure fixtures exist
    assert!(fs::metadata(p_json).is_ok());
    assert!(fs::metadata(p_yaml).is_ok());

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args(["--no-color", "-n", "10000", "-f", "auto", p_json, p_yaml])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);

    // Find the YAML section body
    let header = format!("==> {p_yaml} <==");
    let start = out.find(&header).expect("yaml header present") + header.len();
    // find next header or end
    let next = out[start..]
        .find("==> ")
        .map(|i| start + i)
        .unwrap_or(out.len());
    let section = &out[start..next];

    // First non-empty line should start with a dash (YAML sequence item)
    let first_non_empty = section
        .lines()
        .map(str::trim_start)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    assert!(
        first_non_empty.starts_with('-'),
        "expected YAML sequence items to start with '-': {first_non_empty:?}",
    );
    // Should contain YAML mapping key syntax for one of the objects
    assert!(
        section.contains("name:"),
        "expected YAML key 'name:' present in section: {section:?}"
    );
}
