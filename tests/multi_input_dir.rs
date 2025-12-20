mod common;
use std::fs;
use tempfile::tempdir;

fn run_with_paths_json(paths: &[&str]) -> (bool, String, String) {
    // large budget to avoid truncation
    let mut args =
        vec!["--no-color", "--no-sort", "-c", "100000", "-f", "auto"];
    args.extend_from_slice(paths);
    let output = common::run_cli(&args, None);
    let ok = output.status.success();
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    let err = String::from_utf8_lossy(&output.stderr).into_owned();
    (ok, out, err)
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single test composes setup + assertions succinctly"
)]
fn directory_inputs_are_ignored_and_reported() {
    let dir = tempdir().expect("tmp");
    let sub = dir.path().join("subdir");
    fs::create_dir_all(&sub).expect("mkdir");

    let json = dir.path().join("ok.json");
    fs::write(&json, b"{\"ok\":true}").expect("write json");

    let json_s = json.to_string_lossy().to_string();
    let sub_s = sub.to_string_lossy().to_string();

    let (ok, out, err) = run_with_paths_json(&[&json_s, &sub_s]);
    assert!(ok, "should succeed: {err}");

    assert!(out.contains("==> "));
    assert!(out.contains(&json_s));
    assert!(!out.contains(&format!("==> {sub_s} <==")));

    let err_t = err.trim_end();
    assert!(
        err_t.ends_with(&format!("Ignored directory: {sub_s}")),
        "stderr should end with directory ignore notice. stderr: {err_t}"
    );
}
