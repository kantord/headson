use std::fs;

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "Assertion-heavy integration test; splitting would add indirection with no value."
)]
fn fileset_auto_unknown_extensions_use_text_template() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p1 = dir.path().join("a.txt");
    let p2 = dir.path().join("b.log");
    fs::write(&p1, b"one\ntwo\n").unwrap();
    fs::write(&p2, b"alpha\nbeta\n").unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "-n",
            "10000",
            "-f",
            "auto",
            p1.to_str().unwrap(),
            p2.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    // Section headers
    assert!(out.contains("a.txt"));
    assert!(out.contains("b.log"));
    // Raw lines (no JSON quotes)
    assert!(out.contains("one\n"));
    assert!(!out.contains("\"one\""));
    assert!(out.contains("alpha\n"));
}
