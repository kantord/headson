fn run_json(paths: &[&str], budget: usize) -> String {
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let mut args = vec!["--no-color", "-n", &budget_s, "-f", "json"];
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[test]
fn json_fileset_sections_headers_present() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let out = run_json(&[p1, p2], 100_000);
    assert!(out.contains("==> "));
    assert!(out.contains("object_small.json"));
    assert!(out.contains("array_numbers_50.json"));
}

#[test]
fn json_fileset_small_budget_shows_summary() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    let out = run_json(&[p1, p2, p3], 50);
    assert!(out.contains("more files"));
}
