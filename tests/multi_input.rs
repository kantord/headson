use serde_json::Value;

fn run_with_paths(paths: &[&str], budget: usize) -> (bool, String, String) {
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    // Force JSON template to allow JSON parsing assertions
    let mut args = vec!["--no-color", "-n", &budget_s, "-f", "json"];
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert();
    let ok = assert.get_output().status.success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let err =
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    (ok, out, err)
}

fn assert_no_special_formatting(out: &str) {
    assert!(
        !out.contains("==>"),
        "json template must not include headers"
    );
    assert!(
        !out.contains("<=="),
        "json template must not include headers"
    );
    assert!(
        !out.lines().any(|l| l.trim_start().starts_with("// ")),
        "json template must not include JS-style comments"
    );
}

fn parse_and_assert_keys(out: &str, p1: &str, p2: &str) {
    let v: Value =
        serde_json::from_str(out).expect("json output should parse");
    let obj = v.as_object().expect("root should be object for multi-file");
    assert!(obj.contains_key(p1), "object should contain first path");
    assert!(obj.contains_key(p2), "object should contain second path");
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

#[test]
fn json_template_has_no_special_formatting_for_multi_file() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let (ok, out, err) = run_with_paths(&[p1, p2], 100_000);
    assert!(ok, "multi path should succeed: {err}");
    // Output must be valid JSON (no headers, comments, or other adornments)
    assert_no_special_formatting(&out);
    parse_and_assert_keys(&out, p1, p2);
}
