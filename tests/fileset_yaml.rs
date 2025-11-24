fn run_yaml(paths: &[&str], budget: usize) -> String {
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    // newline mode
    let mut args =
        vec!["--no-color", "--no-sort", "-c", &budget_s, "-f", "auto"];
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[test]
fn yaml_fileset_sections_headers_present() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let out = run_yaml(&[p1, p2], 100_000);
    assert!(out.contains("==> "));
    assert!(out.contains("object_small.json"));
    assert!(out.contains("array_numbers_50.json"));
}

#[test]
fn yaml_fileset_omitted_summary_when_budget_small() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    let budget = 30usize;
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--no-color",
            "--no-sort",
            "-H",
            "-c",
            &budget_s,
            "-f",
            "auto",
            p1,
            p2,
            p3,
        ])
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(out.contains("more files"));
}

#[test]
fn yaml_compact_falls_back_to_json_style() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let budget = 500usize;
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    // Compact => no newlines; YAML template renders via JSON style
    let assert = cmd
        .args([
            "--no-color",
            "--no-sort",
            "-c",
            &budget_s,
            "-f",
            "auto",
            "--compact",
            p1,
            p2,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(out.contains("{"), "expected JSON-style compact rendering");
    let trimmed = out.trim_end_matches('\n');
    assert!(
        !trimmed.contains('\n'),
        "expected no internal newlines in compact output: {out:?}"
    );
}

#[test]
fn yaml_fileset_compact_snapshot() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let budget = 500usize;
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--no-color",
            "--no-sort",
            "-c",
            &budget_s,
            "-f",
            "auto",
            "--compact",
            p1,
            p2,
        ])
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_snapshot!("yaml_fileset_compact", out);
}
use insta::assert_snapshot;
