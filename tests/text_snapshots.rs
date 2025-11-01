// Intentionally not importing prelude wildcard; using cargo_bin_cmd! macro.

#[test]
fn text_stdin_snapshot() {
    let input = b"a\r\nb\r\nc\r\n".to_vec();
    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
        .args(["--no-color", "-i", "text", "-f", "text"])
        .write_stdin(input)
        .assert()
        .success();
    let output = assert.get_output().clone();
    let mut out = String::from_utf8_lossy(&output.stdout).to_string();
    // Normalize trailing newlines to a single one for snapshot stability.
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    insta::assert_snapshot!(out);
}

#[test]
fn fileset_text_files_snapshot() {
    let dir = tempfile::tempdir().expect("tmpdir");
    std::fs::write(dir.path().join("a.txt"), b"one\ntwo\n").unwrap();
    std::fs::write(dir.path().join("b.log"), b"alpha\nbeta\n").unwrap();

    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
        .current_dir(dir.path())
        .args(["--no-color", "-n", "10000", "-f", "auto", "a.txt", "b.log"])
        .assert()
        .success();
    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    insta::assert_snapshot!(out);
}
