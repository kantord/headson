fn run_pseudo(paths: &[&str], budget: usize) -> String {
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let mut args = vec!["--no-color", "-n", &budget_s, "-f", "pseudo"]; // newline mode
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[test]
fn pseudo_fileset_inline_object_no_section_headers() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    let out = run_pseudo(&[p1, p2, p3], 100_000);
    assert!(out.trim_start().starts_with('{'));
    assert!(
        !out.contains("==>"),
        "should not contain pseudo-style section headers"
    );
}

#[test]
fn pseudo_fileset_shows_omission_marker_when_budget_small() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    // Tiny budget to force omission; inline pseudo uses ellipsis markers.
    let out = run_pseudo(&[p1, p2, p3], 50);
    assert!(out.contains('…') || out.contains("..."));
}

#[test]
fn pseudo_fileset_compact_shows_ellipsis_for_omitted() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    let budget = 50usize;
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    // Compact mode => object-style rendering; expect ellipsis for omitted content
    let assert = cmd
        .args([
            "--no-color",
            "-n",
            &budget_s,
            "-f",
            "pseudo",
            "--compact",
            p1,
            p2,
            p3,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains('…') || out.contains("..."),
        "expected ellipsis marker for omitted content: {out:?}"
    );
}
