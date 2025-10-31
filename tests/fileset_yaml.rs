use insta::assert_snapshot;

fn run_yaml(paths: &[&str], budget: usize) -> String {
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    // newline mode
    let mut args = vec!["--no-color", "-n", &budget_s, "-f", "yaml"];
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[test]
fn yaml_fileset_renders_mapping() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let out = run_yaml(&[p1, p2], 100_000);
    // Expect file names present (keys are quoted full paths in YAML)
    assert!(out.contains("object_small.json"));
    assert!(out.contains("array_numbers_50.json"));
    assert_snapshot!("yaml_fileset_mapping", out);
}

#[test]
fn yaml_fileset_omitted_summary_when_budget_small() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    let out = run_yaml(&[p1, p2, p3], 50);
    assert!(
        out.contains("more files"),
        "expected omitted summary comment for files: {out:?}"
    );
}

#[test]
fn yaml_compact_falls_back_to_json_style() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let budget = 500usize;
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    // Compact => no newlines; YAML template renders via JSON style
    let assert = cmd
        .args([
            "--no-color",
            "-n",
            &budget_s,
            "-f",
            "yaml",
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
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "-n",
            &budget_s,
            "-f",
            "yaml",
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
