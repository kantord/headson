use std::fs;

#[test]
fn single_file_auto_unknown_ext_defaults_to_indent() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p = dir.path().join("data.txt");
    fs::write(&p, b"alpha\nbeta\ngamma\n").unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "-c",
            "10000",
            "-f",
            "auto",
            p.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    // Default is indent-structured text output; should contain raw lines
    assert!(out.contains("alpha\n"));
    assert!(!out.contains("\"alpha\""));
}
