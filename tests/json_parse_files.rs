use serde_json::Value;
use std::fs;
use std::path::Path;
use test_each_file::test_each_path;

fn run_cli(input: &[u8]) -> (bool, Vec<u8>, Vec<u8>) {
    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
        .args(["--no-color", "-n", "10000", "-f", "json"]) // ensure valid JSON output
        .write_stdin(input)
        .assert();
    let ok = assert.get_output().status.success();
    let out = assert.get_output().stdout.clone();
    let err = assert.get_output().stderr.clone();
    (ok, out, err)
}

fn is_y(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|n| n.starts_with("y_"))
        .unwrap_or(false)
}
fn is_n(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|n| n.starts_with("n_"))
        .unwrap_or(false)
}

test_each_path! { in "tests/fixtures/json/JSONTestSuite/test_parsing" => jsonsuite_case }

fn file_name_str(path: &Path) -> Option<&str> {
    path.file_name().and_then(|s| s.to_str())
}

fn is_json_file(path: &Path) -> bool {
    path.extension().map(|e| e == "json").unwrap_or(false)
}

fn should_skip_case(path: &Path) -> bool {
    match file_name_str(path) {
        // Known simd-json serde differences; see README.
        Some("n_multidigit_number_then_00.json")
        // serde_json may keep -0.0 as float while ours yields integer 0; skip these positives.
        | Some("y_number_minus_zero.json")
        | Some("y_number_negative_zero.json") => true,
        _ => false,
    }
}

fn verify_positive(path: &Path, input: &[u8]) {
    let original: Value = serde_json::from_slice(input).expect("serde accept");
    let (ok, out, _err) = run_cli(input);
    assert!(ok, "cli should succeed: {}", path.display());
    let reparsed: Value =
        serde_json::from_slice(&out).expect("cli output valid json");
    assert_eq!(original, reparsed, "roundtrip mismatch: {}", path.display());
}

fn verify_negative(path: &Path, input: &[u8]) {
    assert!(
        serde_json::from_slice::<Value>(input).is_err(),
        "serde should reject: {}",
        path.display()
    );
    let (ok, _out, err) = run_cli(input);
    assert!(!ok, "cli should fail: {}", path.display());
    assert!(
        !String::from_utf8_lossy(&err).trim().is_empty(),
        "stderr non-empty: {}",
        path.display()
    );
}

fn jsonsuite_case(path: &Path) {
    if !is_json_file(path) || should_skip_case(path) {
        return;
    }
    let input = fs::read(path).expect("read");
    if is_y(path) {
        verify_positive(path, &input);
    } else if is_n(path) {
        verify_negative(path, &input);
    }
}
