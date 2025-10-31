fn run_js(paths: &[&str], budget: usize) -> String {
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let mut args = vec!["--no-color", "-n", &budget_s, "-f", "js"]; // newline mode
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[test]
fn js_fileset_inline_object_no_section_headers() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    let out = run_js(&[p1, p2, p3], 100_000);
    assert!(out.trim_start().starts_with('{'));
    assert!(
        !out.contains("\n// "),
        "should not contain section header comments"
    );
    assert!(
        !out.contains("==>"),
        "should not contain pseudo-style header markers"
    );
}

#[test]
fn js_fileset_shows_omitted_summary_when_budget_small() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    // Use a tiny budget to ensure some files are omitted
    let out = run_js(&[p1, p2, p3], 50);
    assert!(
        out.contains("more files"),
        "expected omitted summary in output: {out:?}"
    );
}

#[test]
fn js_fileset_compact_shows_inline_omitted_summary() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    let budget = 50usize;
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    // Compact mode => no newlines, but object-style rendering includes inline summary
    let assert = cmd
        .args([
            "--no-color",
            "-n",
            &budget_s,
            "-f",
            "js",
            "--compact",
            p1,
            p2,
            p3,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("more files"),
        "expected inline summary: {out:?}"
    );
}

#[test]
fn js_fileset_small_budget_shows_omission_comment() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let out = run_js(&[p1, p2], 1);
    assert!(out.contains("/*"), "expected omission comment in output");
}
