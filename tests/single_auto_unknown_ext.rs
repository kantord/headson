use std::fs;

#[test]
fn single_file_auto_unknown_ext_defaults_to_text() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p = dir.path().join("data.txt");
    fs::write(&p, b"alpha\nbeta\ngamma\n").unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "-n",
            "10000",
            "-f",
            "auto",
            p.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    // headson prints the rendered output and adds a trailing println newline.
    // Text template emitted a newline per line; println adds one more.
    assert_eq!(out, "alpha\nbeta\ngamma\n\n");
}
