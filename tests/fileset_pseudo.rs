use insta::assert_snapshot;

fn run_pseudo(paths: &[&str], budget: usize) -> String {
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let mut args = vec!["--no-color", "-n", &budget_s, "-f", "pseudo"]; // newline mode
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[test]
fn pseudo_fileset_head_style_headers() {
    // Use three reasonably small fixtures
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    // Large budget to include content; we care about headers/separators
    let out = run_pseudo(&[p1, p2, p3], 100_000);
    assert_snapshot!("pseudo_fileset_head_style_headers", out);
}

#[test]
fn pseudo_fileset_shows_omitted_summary_when_budget_small() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    // Tiny budget to force omission of some files
    let out = run_pseudo(&[p1, p2, p3], 50);
    assert!(
        out.contains("more files"),
        "expected omitted summary in output: {out:?}"
    );
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
