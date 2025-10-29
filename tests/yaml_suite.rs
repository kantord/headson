use assert_cmd::Command;
use serde_json::Value as J;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use test_each_file::test_each_path;
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

fn run_cli_yaml(input: &[u8]) -> (bool, String, String) {
    let assert = Command::cargo_bin("headson")
        .unwrap()
        .args([
            "--no-color",
            "-n",
            "1000000",
            "--string-cap",
            "1000000",
            "-f",
            "yaml",
            "-i",
            "yaml",
        ]) // parse YAML, render YAML with no truncation
        .write_stdin(input)
        .assert();
    let ok = assert.get_output().status.success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let err =
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    (ok, out, err)
}

fn is_yaml_file(path: &Path) -> bool {
    path.extension().map(|e| e == "yaml").unwrap_or(false)
}

test_each_path! { in "yaml-test-suite" => yaml_suite_case }

fn yaml_suite_case(path: &Path) {
    if !is_yaml_file(path) {
        return;
    }
    let input = fs::read(path).expect("read yaml");
    let (ok, out, err) = run_cli_yaml(&input);
    assert!(
        ok,
        "cli should succeed for YAML: {}\nerr: {}",
        path.display(),
        err
    );

    // Output should be valid YAML that parses with yaml-rust2 as at least one document.
    let docs = YamlLoader::load_from_str(&out)
        .expect("output should parse via yaml-rust2");
    assert!(
        !docs.is_empty(),
        "expected at least one YAML document in output for {}",
        path.display()
    );

    // Deep semantic equivalence: normalize original and output and compare.
    let orig_docs = YamlLoader::load_from_str(
        std::str::from_utf8(&input).unwrap_or_default(),
    )
    .expect("input YAML parses");

    let norm_in = normalize_docs(&orig_docs);
    let norm_out = normalize_docs(&docs);
    assert_eq!(
        norm_in,
        norm_out,
        "normalized YAML mismatch for {}\n--- in:\n{:?}\n--- out:\n{:?}",
        path.display(),
        norm_in,
        norm_out
    );
}

fn normalize_docs(docs: &[Yaml]) -> J {
    if docs.is_empty() {
        // Treat empty input as empty array to match current ingest behavior.
        return J::Array(vec![]);
    }
    if docs.len() == 1 {
        return normalize_yaml(&docs[0]);
    }
    J::Array(docs.iter().map(normalize_yaml).collect())
}

fn normalize_yaml(y: &Yaml) -> J {
    match y {
        Yaml::Null | Yaml::BadValue => J::Null,
        Yaml::Boolean(b) => J::Bool(*b),
        // Keep numeric tokens as strings to avoid representation diffs
        Yaml::Integer(i) => J::String(i.to_string()),
        Yaml::Real(s) | Yaml::String(s) => J::String(s.clone()),
        Yaml::Alias(n) => J::String(format!("*{n}")),
        Yaml::Array(v) => J::Array(v.iter().map(normalize_yaml).collect()),
        Yaml::Hash(map) => {
            let mut obj: BTreeMap<String, J> = BTreeMap::new();
            for (k, v) in map.iter() {
                let kk = stringify_yaml_key(k);
                obj.insert(kk, normalize_yaml(v));
            }
            // Convert to serde_json::Value::Object
            J::Object(obj.into_iter().collect())
        }
    }
}

fn stringify_yaml_key(y: &Yaml) -> String {
    if let Yaml::String(s) = y {
        return s.clone();
    }
    let mut out = String::new();
    {
        let mut emitter = YamlEmitter::new(&mut out);
        let _ = emitter.dump(y);
    }
    out.replace('\n', " ")
}
