use assert_cmd::Command;
use insta::assert_snapshot;
use std::fs;
use std::path::Path;

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

const BUDGET_TIGHT: usize = 120; // significantly truncated but readable
const BUDGET_MED: usize = 600; // slightly truncated for many
const BUDGET_FULL: usize = 1_000_000; // effectively untruncated

#[test]
fn yaml_first_five_snapshots() {
    let root = Path::new("tests/fixtures/yaml/yaml-test-suite");
    let mut files: Vec<_> = fs::read_dir(root)
        .expect("read fixture dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| is_yaml_file(p))
        .collect();
    files.sort();
    for path in files.into_iter().take(5) {
        let input = fs::read(&path).expect("read yaml");
        let name = stem_str(&path);
        let tight = run_cli_yaml_with_budget(&input, BUDGET_TIGHT);
        assert_snapshot!(format!("yaml_first5_{}_tight", name), tight);
        let med = run_cli_yaml_with_budget(&input, BUDGET_MED);
        assert_snapshot!(format!("yaml_first5_{}_med", name), med);
        let full = run_cli_yaml_with_budget(&input, BUDGET_FULL);
        assert_snapshot!(format!("yaml_first5_{}_full", name), full);
    }
}

// No output normalization: runtime behavior is deterministic (aliases -> "*alias").
