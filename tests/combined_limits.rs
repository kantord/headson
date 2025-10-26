use assert_cmd::Command;
use insta::assert_snapshot;

fn run_args(args: &[&str]) -> String {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd.args(args).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[test]
fn combined_limits_across_multiple_files_matches_minimum_global() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    // -n 300, -N 120 => effective global limit 120
    let out_both = run_args(&["-f", "json", "-n", "300", "-N", "120", p1, p2]);
    let out_min_only = run_args(&["-f", "json", "-N", "120", p1, p2]);
    assert_eq!(out_both, out_min_only, "-n + -N should equal -N=min(n,N)");
    assert_snapshot!("combined_limits_two_files_json_min120", out_both);
}

#[test]
fn combined_limits_single_file_honors_per_file_minimum() {
    let p = "tests/fixtures/explicit/string_escaping.json";
    // -n 80, -N 200 => effective global limit 80
    let out_both = run_args(&["-f", "pseudo", "-n", "80", "-N", "200", p]);
    let out_min_only = run_args(&["-f", "pseudo", "-N", "80", p]);
    assert_eq!(out_both, out_min_only, "-n + -N should equal -N=min(n,N)");
    assert_snapshot!("combined_limits_single_file_pseudo_min80", out_both);
}

#[test]
fn combined_limits_many_files_use_aggregate_per_file_budget() {
    use std::fs;
    let tmp = tempfile::tempdir().expect("tmp");
    let mut paths = Vec::new();
    for i in 0..8 {
        let p = tmp.path().join(format!("f{i}.json"));
        fs::write(&p, b"[1,2,3]").unwrap();
        paths.push(p);
    }
    let path_strs: Vec<String> =
        paths.iter().map(|p| p.to_string_lossy().into()).collect();
    // Per-file budget 40; with 8 files aggregate=320. Global 1000 should not constrain.
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let mut args = vec!["-f", "js", "-n", "40", "-N", "1000"]; // newline mode
    for s in &path_strs {
        args.push(s);
    }
    let assert = cmd.args(args).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    // Expect all files are included (no omitted summary)
    assert!(
        !out.contains("more files"),
        "should not omit files under aggregate per-file budget: {out:?}"
    );
    // Count JS headers
    let headers = out
        .lines()
        .filter(|l| l.trim_start().starts_with("// "))
        .count();
    assert_eq!(headers, path_strs.len());
}
