mod common;
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
    expect_fail: bool,
) -> common::CliOutput {
    let budget_s = budget.to_string();
    let mut args = vec!["--no-color", "-c", &budget_s];
    let lower = template.to_ascii_lowercase();
    match lower.as_str() {
        "json" => args.extend(["-f", "json"]),
        "yaml" => args.extend(["-f", "yaml", "-i", "yaml"]),
        "pseudo" => args.extend(["-f", "json", "-t", "default"]),
        "js" => args.extend(["-f", "json", "-t", "detailed"]),
        other => args.extend(["-f", other]),
    }
    args.push(path);
    args.extend_from_slice(extra);
    if expect_fail {
        common::run_cli_expect_fail(&args, None, None)
    } else {
        common::run_cli(&args, None)
    }
}

#[test]
fn stdin_and_input_path_produce_identical_output() {
    let path = "tests/fixtures/explicit/object_small.json";
    let input = fs::read_to_string(path).expect("read fixture");
    let templates = ["json", "pseudo", "js"];
    let budget = 1000usize;
    for &tmpl in &templates {
        let out_stdin =
            common::run_template_budget_no_color(&input, tmpl, budget, &[]);
        let out = run_with_input_path(path, tmpl, budget, &[], false);
        assert_eq!(out_stdin, out.stdout, "tmpl={tmpl}");
    }
}

#[test]
fn unreadable_file_path_errors_with_stderr() {
    let out = run_with_input_path("/no/such/file", "json", 100, &[], true);
    assert!(!out.stderr.trim().is_empty(), "stderr should be non-empty");
}

#[test]
fn directories_and_binary_files_are_ignored_with_warnings() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");

    let dir_path = tmpdir.path().join("subdir");
    create_subdir(&dir_path);

    let bin_path = tmpdir.path().join("bin.dat");
    write_binary_file(&bin_path);

    let json_path = tmpdir.path().join("data.json");
    write_json_file(&json_path, b"{\"a\":1}");

    let output = common::run_cli(
        &[
            "--no-color",
            "-c",
            "100",
            "-f",
            "auto",
            json_path.to_str().unwrap(),
            dir_path.to_str().unwrap(),
            bin_path.to_str().unwrap(),
        ],
        None,
    );

    let out = output.stdout;
    let err = output.stderr;
    assert!(out.contains("\n") || out.contains('{'));
    assert!(
        err.contains("Ignored directory:")
            && err.contains("Ignored binary file:"),
        "stderr should contain ignore warnings, got: {err:?}"
    );
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single test covers two flows succinctly"
)]
fn only_ignored_inputs_result_in_empty_output_and_warnings() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");

    let dir_path = tmpdir.path().join("subdir");
    create_subdir(&dir_path);
    let bin_path = tmpdir.path().join("bin.dat");
    write_binary_file(&bin_path);

    // Case 1: single ignored path -> falls into included == 0 branch, empty output
    let output1 = common::run_cli(
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "100",
            "-f",
            "auto",
            dir_path.to_str().unwrap(),
        ],
        None,
    );
    let out1 = output1.stdout;
    let err1 = output1.stderr;
    assert_eq!(out1, "\n", "expected empty output when nothing included");
    assert!(err1.contains("Ignored directory:"));

    // Case 2: multiple ignored paths -> no included inputs, empty output
    let output2 = common::run_cli(
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "100",
            "-f",
            "auto",
            dir_path.to_str().unwrap(),
            bin_path.to_str().unwrap(),
        ],
        None,
    );
    let out2 = output2.stdout;
    let err2 = output2.stderr;
    assert_eq!(out2, "\n", "expected empty output when nothing included");
    assert!(
        err2.contains("Ignored directory:")
            && err2.contains("Ignored binary file:"),
        "stderr should contain both ignore warnings, got: {err2:?}"
    );
}

#[test]
fn global_budget_limits_total_output_vs_per_file_budget() {
    // Two inputs; with -c 40 the effective budget is per-file (40) * 2 => 80.
    // With --global-bytes 40, the total budget is capped at 40.
    let tmp = tempfile::tempdir().expect("tmp");
    let a = tmp.path().join("a.json");
    let b = tmp.path().join("b.json");
    // Simple arrays long enough to show a budget difference
    fs::write(&a, b"[1,2,3,4,5,6,7,8,9,10]").unwrap();
    fs::write(&b, b"[1,2,3,4,5,6,7,8,9,10]").unwrap();

    // Per-file budget (-c) scenario
    let out_pf = {
        let args = [
            "--no-color",
            "--no-sort",
            "-c",
            "40",
            "-f",
            "auto",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ];
        common::run_cli(&args, None).stdout
    };

    // Global budget scenario
    let out_g = {
        let args = [
            "--no-color",
            "--no-sort",
            "--global-bytes",
            "40",
            "-f",
            "auto",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ];
        common::run_cli(&args, None).stdout
    };

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
