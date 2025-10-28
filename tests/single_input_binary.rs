use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn single_binary_file_is_ignored_with_notice() {
    let dir = tempdir().expect("tmp");
    let bin = dir.path().join("binfile");
    fs::write(&bin, [0x00, 0x01, 0xFF, 0xFE]).expect("write bin");

    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .arg("--no-color")
        .arg(bin.to_string_lossy().to_string())
        .assert();

    let status = assert.get_output().status.success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let err =
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    assert!(status, "should succeed");
    assert!(
        out.trim().is_empty(),
        "stdout should be empty for ignored file"
    );
    let bin_s = bin.to_string_lossy().to_string();
    assert!(
        err.trim_end()
            .ends_with(&format!("Ignored binary file: {bin_s}")),
        "stderr should end with ignore notice: {err}"
    );
}
