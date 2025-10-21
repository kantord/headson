#[path = "../test_support/mod.rs"]
mod util;
use assert_cmd::Command;
use std::fs;

fn run_with_input_path(
    path: &str,
    template: &str,
    budget: usize,
    extra: &[&str],
) -> (bool, String, String) {
    let budget_s = budget.to_string();
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let mut args = vec!["-n", &budget_s, "-f", template, "--input", path];
    args.extend_from_slice(extra);
    let assert = cmd.args(args).assert();
    let ok = assert.get_output().status.success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let err =
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    (ok, out, err)
}

#[test]
fn stdin_and_input_path_produce_identical_output() {
    let path = "tests/fixtures/explicit/object_small.json";
    let input = fs::read_to_string(path).expect("read fixture");
    let templates = ["json", "pseudo", "js"];
    let budget = 1000usize;
    for &tmpl in &templates {
        let out_stdin = util::run_template_budget(&input, tmpl, budget, &[]);
        let (ok, out_file, err) = run_with_input_path(path, tmpl, budget, &[]);
        assert!(ok, "cli should succeed (tmpl={tmpl}): {err}");
        assert_eq!(out_stdin, out_file, "tmpl={tmpl}");
    }
}

#[test]
fn unreadable_file_path_errors_with_stderr() {
    let (ok, _out, err) =
        run_with_input_path("/no/such/file", "json", 100, &[]);
    assert!(!ok, "cli should fail for unreadable file");
    assert!(!err.trim().is_empty(), "stderr should be non-empty");
}
