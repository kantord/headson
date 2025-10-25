#[path = "../test_support/mod.rs"]
mod util;
use assert_cmd::Command;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn create_subdir(path: &Path) {
    fs::create_dir(path).expect("mkdir");
}

fn write_binary_file(path: &Path) {
    let mut f = File::create(path).expect("create bin");
    f.write_all(&[0, 159, 146, 150, 0, 0]).expect("write bin");
}

fn write_json_file(path: &Path, contents: &[u8]) {
    fs::write(path, contents).expect("write json");
}

fn run_with_input_path(
    path: &str,
    template: &str,
    budget: usize,
    extra: &[&str],
) -> (bool, String, String) {
    let budget_s = budget.to_string();
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let mut args = vec!["-n", &budget_s, "-f", template, path];
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

#[test]
fn directories_and_binary_files_are_ignored_with_notices() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");

    let dir_path = tmpdir.path().join("subdir");
    create_subdir(&dir_path);

    let bin_path = tmpdir.path().join("bin.dat");
    write_binary_file(&bin_path);

    let json_path = tmpdir.path().join("data.json");
    write_json_file(&json_path, b"{\"a\":1}");

    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .args([
            "-n",
            "100",
            "-f",
            "json",
            json_path.to_str().unwrap(),
            dir_path.to_str().unwrap(),
            bin_path.to_str().unwrap(),
        ])
        .assert();

    let ok = assert.get_output().status.success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(ok, "cli should succeed: {err}");
    assert!(out.contains("\n") || out.contains('{'));
    assert!(
        err.contains("Ignored directory:")
            && err.contains("Ignored binary file:"),
        "stderr should contain ignore notices, got: {err:?}"
    );
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single test covers two flows succinctly"
)]
fn only_ignored_inputs_result_in_empty_output_and_notices() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");

    let dir_path = tmpdir.path().join("subdir");
    create_subdir(&dir_path);
    let bin_path = tmpdir.path().join("bin.dat");
    write_binary_file(&bin_path);

    // Case 1: single ignored path -> falls into included == 0 branch, empty output
    let mut cmd1 = Command::cargo_bin("headson").expect("bin");
    let assert1 = cmd1
        .args(["-n", "100", "-f", "json", dir_path.to_str().unwrap()])
        .assert();
    let ok1 = assert1.get_output().status.success();
    let out1 = String::from_utf8_lossy(&assert1.get_output().stdout);
    let err1 = String::from_utf8_lossy(&assert1.get_output().stderr);
    assert!(ok1, "cli should succeed: {err1}");
    assert_eq!(out1, "\n", "expected empty output when nothing included");
    assert!(err1.contains("Ignored directory:"));

    // Case 2: multiple ignored paths -> fileset mode renders empty object
    let mut cmd2 = Command::cargo_bin("headson").expect("bin");
    let assert2 = cmd2
        .args([
            "-n",
            "100",
            "-f",
            "json",
            dir_path.to_str().unwrap(),
            bin_path.to_str().unwrap(),
        ])
        .assert();
    let ok2 = assert2.get_output().status.success();
    let out2 = String::from_utf8_lossy(&assert2.get_output().stdout);
    let err2 = String::from_utf8_lossy(&assert2.get_output().stderr);
    assert!(ok2, "cli should succeed: {err2}");
    assert_eq!(
        out2, "{}\n",
        "expected empty fileset object when multiple inputs"
    );
    assert!(
        err2.contains("Ignored directory:")
            && err2.contains("Ignored binary file:"),
        "stderr should contain both ignore notices, got: {err2:?}"
    );
}

#[test]
fn global_budget_limits_total_output_vs_per_file_budget() {
    // Two inputs; with -n 40 the effective budget is per-file (40) * 2 => 80.
    // With --global-budget 40, the total budget is capped at 40.
    let tmp = tempfile::tempdir().expect("tmp");
    let a = tmp.path().join("a.json");
    let b = tmp.path().join("b.json");
    // Simple arrays long enough to show a budget difference
    fs::write(&a, b"[1,2,3,4,5,6,7,8,9,10]").unwrap();
    fs::write(&b, b"[1,2,3,4,5,6,7,8,9,10]").unwrap();

    // Per-file budget (-n) scenario
    let mut cmd_pf = Command::cargo_bin("headson").expect("bin");
    let assert_pf = cmd_pf
        .args([
            "-n",
            "40",
            "-f",
            "json",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out_pf =
        String::from_utf8_lossy(&assert_pf.get_output().stdout).into_owned();

    // Global budget scenario
    let mut cmd_g = Command::cargo_bin("headson").expect("bin");
    let assert_g = cmd_g
        .args([
            "--global-budget",
            "40",
            "-f",
            "json",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out_g =
        String::from_utf8_lossy(&assert_g.get_output().stdout).into_owned();

    assert!(
        out_g.len() <= out_pf.len(),
        "global budget should not exceed per-file budget total: global={}, per-file={}",
        out_g.len(),
        out_pf.len()
    );
    assert!(
        out_g.len() < out_pf.len(),
        "expected global budget output to be strictly shorter for these inputs"
    );
}
