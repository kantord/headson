use insta::assert_snapshot;

fn run_args(args: &[&str]) -> String {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd.arg("--no-color").args(args).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

fn make_tmp_with_files(count: usize) -> (tempfile::TempDir, Vec<String>) {
    use std::fs;
    let tmp = tempfile::tempdir_in(".").expect("tmp");
    let mut names: Vec<String> = Vec::with_capacity(count);
    for i in 0..count {
        let name = format!("a{i}.json");
        let p = tmp.path().join(&name);
        fs::write(&p, b"{}\n").unwrap();
        names.push(name);
    }
    (tmp, names)
}

fn run_fileset_json_with_budgets(
    dir: &std::path::Path,
    names: &[String],
    per_file: usize,
    global: usize,
) -> serde_json::Value {
    use serde_json::Value;
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let mut args: Vec<String> = vec![
        "--no-color".into(),
        "-f".into(),
        "json".into(),
        "-n".into(),
        per_file.to_string(),
        "-N".into(),
        global.to_string(),
    ];
    for s in names {
        args.push(s.clone());
    }
    cmd.current_dir(dir);
    let assert = cmd.args(args).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    serde_json::from_str::<Value>(&out).expect("json parse")
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
    let (tmp, names) = make_tmp_with_files(8);
    let v = run_fileset_json_with_budgets(tmp.path(), &names, 40, 1000);
    let obj = v.as_object().expect("root object");
    assert_eq!(obj.len(), names.len(), "should include all files");
    assert!(names.iter().all(|n| obj.contains_key(n)));
}
