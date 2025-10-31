use std::fs;

#[test]
fn single_file_auto_unknown_ext_renders_pseudo_and_uses_json_ingest() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p = dir.path().join("data.txt");
    // Valid JSON content so JSON ingest succeeds
    fs::write(&p, b"{\n  \"a\": 1, \"b\": 2, \"c\": 3\n}\n").unwrap();

    // Use a small budget to encourage omission so Pseudo shows an ellipsis.
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args(["--no-color", "-n", "2", "-f", "auto", p.to_str().unwrap()])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    // Pseudo prints an ellipsis marker when properties are omitted.
    assert!(
        out.contains('…'),
        "expected omission ellipsis in output: {out:?}"
    );
}
