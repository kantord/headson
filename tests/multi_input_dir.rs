use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

fn run_with_paths_json(paths: &[&str]) -> (bool, String, String) {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    // large budget to avoid truncation
    let mut args = vec!["--no-color", "-n", "100000", "-f", "json"];
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert();
    let ok = assert.get_output().status.success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let err =
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    (ok, out, err)
}

#[test]
fn directory_inputs_are_ignored_and_reported() {
    let dir = tempdir().expect("tmp");
    let sub = dir.path().join("subdir");
    fs::create_dir_all(&sub).expect("mkdir");

    let json = dir.path().join("ok.json");
    fs::write(&json, b"{\"ok\":true}").expect("write json");

    let json_s = json.to_string_lossy().to_string();
    let sub_s = sub.to_string_lossy().to_string();

    let (ok, out, err) = run_with_paths_json(&[&json_s, &sub_s]);
    assert!(ok, "should succeed: {err}");

    let v: Value = serde_json::from_str(&out).expect("json parses");
    let obj = v.as_object().expect("root obj");
    assert!(obj.contains_key(&json_s));
    assert!(!obj.contains_key(&sub_s));

    let err_t = err.trim_end();
    assert!(
        err_t.ends_with(&format!("Ignored directory: {sub_s}")),
        "stderr should end with directory ignore notice. stderr: {err_t}"
    );
}
