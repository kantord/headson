use assert_cmd::Command;
use serde_json::Value;

#[path = "../test_support/mod.rs"]
mod util;

fn run_paths_json(paths: &[&str], args: &[&str]) -> (bool, String, String) {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let mut full_args = vec!["-f", "json"];
    full_args.extend_from_slice(args);
    full_args.extend_from_slice(paths);
    let assert = cmd.args(full_args).assert();
    let ok = assert.get_output().status.success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let err =
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    (ok, out, err)
}

#[test]
fn global_limit_can_omit_entire_files() {
    let paths = [
        "tests/fixtures/explicit/array_numbers_50.json",
        "tests/fixtures/explicit/object_small.json",
        "tests/fixtures/explicit/string_escaping.json",
    ];
    // Impose a small global limit so not all files fit.
    let (ok, out, err) = run_paths_json(&paths, &["-N", "120"]);
    assert!(ok, "should succeed: {err}");
    let v: Value = serde_json::from_str(&out).expect("json parse");
    let obj = v.as_object().expect("root object");
    assert!(
        obj.len() < paths.len(),
        "expected some files omitted: {out}"
    );
}

#[test]
fn budget_and_global_limit_conflict() {
    let paths = ["tests/fixtures/explicit/object_small.json"];
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .args(["-f", "json", "-n", "200", "-N", "100", paths[0]])
        .assert();
    assert!(!assert.get_output().status.success(), "should fail");
}
