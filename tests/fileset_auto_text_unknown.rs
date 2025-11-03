use std::fs;

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "Assertion-heavy integration test; splitting would add indirection with no value."
)]
fn fileset_unknown_extensions_force_text_template_when_requested() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p1 = dir.path().join("a.txt");
    let p2 = dir.path().join("b.log");
    fs::write(&p1, b"one\ntwo\n").unwrap();
    fs::write(&p2, b"alpha\nbeta\n").unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "-c",
            "10000",
            "-i",
            "text",
            "-f",
            "text",
            p1.to_str().unwrap(),
            p2.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    // Section headers
    assert!(out.contains("a.txt"));
    assert!(out.contains("b.log"));
    // Raw lines (no JSON quotes) when forcing text ingest/template
    assert!(out.contains("one\n"));
    assert!(!out.contains("\"one\""));
    assert!(out.contains("alpha\n"));
}
