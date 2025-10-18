use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn collect_files(prefix: &str) -> Vec<String> {
    let base = Path::new("JSONTestSuite/test_parsing");
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.starts_with(prefix) && name.ends_with(".json") {
                    if let Some(s) = path.to_str() { files.push(s.to_string()); }
                }
            }
        }
    }
    files.sort();
    files
}

#[test]
fn json_parsing_accepts_roundtrip() {
    let files = collect_files("y_");
    for file in files {
        let input = fs::read_to_string(&file).expect("read file");
        let original: serde_json::Value = serde_json::from_str(&input)
            .unwrap_or_else(|e| panic!("serde should accept {}: {}", file, e));

        let assert = Command::cargo_bin("headson").unwrap()
            .arg("-n").arg("10000")
            .arg("-f").arg("json")
            .write_stdin(input.as_bytes())
            .assert()
            .success();

        let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&output)
            .unwrap_or_else(|e| panic!("output must be valid json for {}: {}\nOUT:\n{}", file, e, output));

        assert_eq!(original, reparsed, "roundtrip mismatch for {}", file);
    }
}

#[test]
fn json_parsing_rejects_fail_cli() {
    let files = collect_files("n_");
    for file in files {
        let input_bytes = fs::read(&file).expect("read file bytes");
        assert!(serde_json::from_slice::<serde_json::Value>(&input_bytes).is_err(), "serde should reject {}", file);

        let assert = Command::cargo_bin("headson").unwrap()
            .arg("-n").arg("10000")
            .arg("-f").arg("json")
            .write_stdin(input_bytes)
            .assert()
            .failure();

        let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
        assert!(!stderr.trim().is_empty(), "stderr should contain error for {}", file);
    }
}
