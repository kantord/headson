use std::fs;

#[test]
fn fileset_rejects_custom_format() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p_a = dir.path().join("a.txt");
    let p_b = dir.path().join("b.txt");
    fs::write(&p_a, "hello").expect("write a");
    fs::write(&p_b, "world").expect("write b");

    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
        .args([
            "--no-color",
            "-f",
            "text",
            "-c",
            "100",
            p_a.to_str().unwrap(),
            p_b.to_str().unwrap(),
        ])
        .current_dir(dir.path())
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("--format cannot be customized for filesets"),
        "stderr missing rejection message: {stderr}"
    );
}
