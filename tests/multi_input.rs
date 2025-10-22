use assert_cmd::Command;
use serde_json::Value;

fn run_with_paths(paths: &[&str], budget: usize) -> (bool, String, String) {
    let budget_s = budget.to_string();
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    // Force JSON template to allow JSON parsing assertions
    let mut args = vec!["-n", &budget_s, "-f", "json"];
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
fn multiple_input_paths_are_wrapped_into_object_root() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    // Use a large budget to include both entries fully
    let (ok, out, err) = run_with_paths(&[p1, p2], 100_000);
    assert!(ok, "multi path should succeed: {err}");
    let v: Value =
        serde_json::from_str(&out).expect("json output should parse");
    let obj = v.as_object().expect("root should be object for multi-file");
    assert!(
        obj.contains_key(p1),
        "object should contain key for first path"
    );
    assert!(
        obj.contains_key(p2),
        "object should contain key for second path"
    );
}
