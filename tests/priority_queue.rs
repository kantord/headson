use assert_cmd::Command;
use insta::assert_snapshot;

#[test]
fn pq_empty_array() {
    let mut cmd = Command::cargo_bin("headson").unwrap();
    let output = cmd.arg("-f").arg("json").write_stdin("[]").output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_snapshot!("pq_empty_array_stderr", stderr);
}

#[test]
fn pq_single_string_array() {
    let mut cmd = Command::cargo_bin("headson").unwrap();
    let output = cmd.arg("-f").arg("json").write_stdin("[\"ab\"]").output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_snapshot!("pq_single_string_array_stderr", stderr);
}
