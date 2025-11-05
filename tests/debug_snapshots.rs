use std::fs;

use serde_json::{self as json, Value};

fn normalize_debug(s: &str) -> String {
    #[allow(
        clippy::cognitive_complexity,
        reason = "small local normalizer for snapshot stability; branching kept inline"
    )]
    fn walk(v: &mut Value) {
        match v {
            Value::Object(map) => {
                if let Some(id) = map.get_mut("id") {
                    *id = Value::from(0);
                }
                if let Some(counts) = map.get_mut("counts") {
                    if let Some(obj) = counts.as_object_mut() {
                        obj.insert("total_nodes".to_string(), Value::from(0));
                        obj.insert("included".to_string(), Value::from(0));
                    }
                }
                for (_k, vv) in map.iter_mut() {
                    walk(vv);
                }
            }
            Value::Array(arr) => {
                for vv in arr.iter_mut() {
                    walk(vv);
                }
            }
            _ => {}
        }
    }
    let mut v: Value = json::from_str(s).expect("stderr must be JSON");
    walk(&mut v);
    json::to_string_pretty(&v).unwrap()
}

#[test]
fn snapshot_debug_json_stdin_strict() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "--debug",
            "-c",
            "200",
            "-f",
            "json",
            "-t",
            "strict",
            "-i",
            "json",
        ]) // strict -> template "json"
        .write_stdin("{\"a\":1,\"b\":{\"c\":2}}\n")
        .assert()
        .success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    let norm = normalize_debug(&err);
    insta::assert_snapshot!("debug_json_stdin_strict", norm);
}

#[test]
fn snapshot_debug_fileset_auto() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p_json = dir.path().join("a.json");
    let p_yaml = dir.path().join("b.yaml");
    fs::write(&p_json, b"{\n  \"a\": 1\n}\n").unwrap();
    fs::write(&p_yaml, b"k: 2\n").unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "--debug",
            "-c",
            "10000",
            "-f",
            "auto",
            "-i",
            "yaml",
            "a.json",
            "b.yaml",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    let norm = normalize_debug(&err);
    insta::assert_snapshot!("debug_fileset_auto", norm);
}
