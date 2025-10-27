use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

fn run_with_paths_json(
    paths: &[&str],
    budget: usize,
) -> (bool, String, String) {
    let budget_s = budget.to_string();
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let mut args = vec!["--no-color", "-n", &budget_s, "-f", "json"];
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert();
    let ok = assert.get_output().status.success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let err =
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    (ok, out, err)
}

#[test]
fn binary_file_is_ignored_and_reported_in_stderr() {
    let dir = tempdir().expect("tempdir");

    let json_path = dir.path().join("data.json");
    fs::write(&json_path, b"{\"a\": 1, \"b\": [1,2,3]}").expect("write json");

    let bin_path = dir.path().join("blob.bin");
    let mut f = fs::File::create(&bin_path).expect("create bin");
    f.write_all(&[0x00, 0xFF, 0x00, 0x01, 0x02, 0x03])
        .expect("write bin");

    let json_s = json_path.to_string_lossy();
    let bin_s = bin_path.to_string_lossy();
    let (ok, out, err) = run_with_paths_json(&[&json_s, &bin_s], 10_000);

    assert!(
        ok,
        "multi-file should succeed even with binary input; stderr: {err}"
    );
    let v: Value = serde_json::from_str(&out).expect("json output parses");
    let obj = v.as_object().expect("root should be object");
    assert!(
        obj.contains_key(&json_s.to_string()),
        "JSON file key present"
    );
    assert!(
        !obj.contains_key(&bin_s.to_string()),
        "binary file key absent"
    );

    let msg = format!("Ignored binary file: {bin_s}");
    let err_trimmed = err.trim_end();
    assert!(
        err_trimmed.ends_with(&msg),
        "stderr should end with expected ignore line\nexpected suffix: {msg}\nstderr: {err_trimmed}"
    );
}

#[allow(
    clippy::cognitive_complexity,
    reason = "test composes several assertions; splitting would reduce clarity"
)]
#[test]
fn multiple_binary_files_each_reported_once_at_end() {
    let dir = tempdir().expect("tempdir");

    let json_path = dir.path().join("ok.json");
    fs::write(&json_path, b"{\"ok\": true}").expect("write json");

    let bin1 = dir.path().join("a.exe");
    fs::write(&bin1, [0x00, 0x01, 0xFF, 0xFE]).expect("write bin1");
    let bin2 = dir.path().join("b.dat");
    fs::write(&bin2, [0x00, 0x00, 0x00, 0xFF, 0x10]).expect("write bin2");

    let json_s = json_path.to_string_lossy().to_string();
    let bin1_s = bin1.to_string_lossy().to_string();
    let bin2_s = bin2.to_string_lossy().to_string();

    let (ok, out, err) =
        run_with_paths_json(&[&json_s, &bin1_s, &bin2_s], 10_000);

    assert!(ok, "should succeed: {err}");
    let v: Value = serde_json::from_str(&out).expect("json output parses");
    let obj = v.as_object().expect("root object");
    assert!(obj.contains_key(&json_s), "json file present");
    assert!(!obj.contains_key(&bin1_s), "bin1 absent");
    assert!(!obj.contains_key(&bin2_s), "bin2 absent");

    let lines: Vec<&str> = err.trim().lines().collect();
    assert!(
        lines.len() >= 2,
        "stderr should have at least two lines: {err}"
    );
    let last_two = &lines[lines.len().saturating_sub(2)..];
    assert_eq!(last_two[0], format!("Ignored binary file: {bin1_s}"));
    assert_eq!(last_two[1], format!("Ignored binary file: {bin2_s}"));
}
