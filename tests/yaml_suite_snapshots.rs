use assert_cmd::Command;
use insta::assert_snapshot;
use std::fs;
use std::path::Path;
use test_each_file::test_each_path;

fn run_cli_yaml_with_budget(input: &[u8], budget: usize) -> String {
    let budget_s = budget.to_string();
    let assert = Command::cargo_bin("headson")
        .unwrap()
        .args([
            "--no-color",
            "-n",
            &budget_s,
            "--string-cap",
            "1000000",
            "-f",
            "yaml",
            "-i",
            "yaml",
        ])
        .write_stdin(input)
        .assert()
        .success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

fn is_yaml_file(path: &Path) -> bool {
    path.extension().map(|e| e == "yaml").unwrap_or(false)
}

fn stem_str(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

const BUDGET_TIGHT: usize = 80; // significantly truncated but readable
const BUDGET_MED: usize = 400; // slightly truncated for many
const BUDGET_FULL: usize = 1_000_000; // effectively untruncated

test_each_path! { in "tests/fixtures/yaml/yaml-test-suite" => yaml_snapshot_case }

fn yaml_snapshot_case(path: &Path) {
    if !is_yaml_file(path) {
        return;
    }
    let input = fs::read(path).expect("read yaml");
    let name = stem_str(path);

    // Tight budget
    let out_tight = run_cli_yaml_with_budget(&input, BUDGET_TIGHT);
    assert_snapshot!(format!("yaml_suite_{}_tight", name), out_tight);

    // Medium budget
    let out_med = run_cli_yaml_with_budget(&input, BUDGET_MED);
    assert_snapshot!(format!("yaml_suite_{}_med", name), out_med);

    // Full budget
    let out_full = run_cli_yaml_with_budget(&input, BUDGET_FULL);
    assert_snapshot!(format!("yaml_suite_{}_full", name), out_full);
}

// No output normalization: runtime behavior is deterministic (aliases -> "*alias").
