use std::{env, fs, io::Write, path::Path};

fn sanitize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() { out.push(ch); } else { out.push('_'); }
    }
    // ensure uniqueness: append a simple hash suffix
    let mut sum: u64 = 0;
    for b in name.as_bytes() { sum = sum.wrapping_mul(131).wrapping_add(*b as u64); }
    out.push_str("_h");
    out.push_str(&format!("{:x}", sum));
    out
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let gen_path = Path::new(&out_dir).join("jsonsuite_tests.rs");

    let mut code = String::new();
    code.push_str("use assert_cmd::Command;\nuse serde_json::Value;\n\n");

    let base = Path::new("JSONTestSuite/test_parsing");
    if let Ok(entries) = fs::read_dir(base) {
        let mut files: Vec<_> = entries.flatten().collect();
        files.sort_by_key(|e| e.file_name());
        for entry in files {
            let path = entry.path();
            if !path.is_file() { continue; }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.ends_with(".json") { continue; }
            if name.starts_with("i_") { continue; }
            let rel = format!("JSONTestSuite/test_parsing/{}", name);
            let fn_name = sanitize(&format!("jsonsuite__{}", name));
            if name.starts_with("y_") {
                code.push_str(&format!(
                    "#[test]\nfn {}() {{\n    let input: &[u8] = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/\", \"{}\"));\n    let original: Value = serde_json::from_slice(input).expect(\"serde should accept\");\n    let assert = Command::cargo_bin(\"headson\").unwrap()\n        .arg(\"-n\").arg(\"10000\")\n        .arg(\"-f\").arg(\"json\")\n        .write_stdin(input)\n        .assert()\n        .success();\n    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();\n    let reparsed: Value = serde_json::from_str(&output).expect(\"output must be valid json\");\n    assert_eq!(original, reparsed);\n}}\n\n",
                    fn_name, rel
                ));
            } else if name.starts_with("n_") {
                code.push_str(&format!(
                    "#[test]\nfn {}() {{\n    let input: &[u8] = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/\", \"{}\"));\n    assert!(serde_json::from_slice::<Value>(input).is_err());\n    let assert = Command::cargo_bin(\"headson\").unwrap()\n        .arg(\"-n\").arg(\"10000\")\n        .arg(\"-f\").arg(\"json\")\n        .write_stdin(input)\n        .assert()\n        .failure();\n    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();\n    assert!(!stderr.trim().is_empty());\n}}\n\n",
                    fn_name, rel
                ));
            }
        }
    }

    fs::File::create(&gen_path).unwrap().write_all(code.as_bytes()).unwrap();
    println!("cargo:rustc-env=JSON_SUITE_GEN={}", gen_path.display());
}
