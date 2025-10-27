use assert_cmd::Command;

#[test]
fn head_and_tail_flags_conflict() {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    // Pass both flags; clap should error with a conflict.
    let assert = cmd
        .args(["--no-color", "--head", "--tail", "-n", "20", "-f", "json"]) // no inputs (stdin not used)
        .assert();
    let ok = assert.get_output().status.success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(!ok, "cli should fail when both --head and --tail are set");
    assert!(
        err.to_ascii_lowercase().contains("conflict")
            || err.contains("cannot be used with"),
        "stderr should mention argument conflict, got: {err}"
    );
}
