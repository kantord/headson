#[test]
fn text_normalizes_bare_cr_to_lf() {
    // Provide only '\r' newlines; expect LF normalization.
    let input = b"a\rb\rc\r".to_vec();
    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
        .args(["--no-color", "-i", "text", "-f", "text", "-n", "1000"])
        .write_stdin(input)
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("a\nb\nc\n"),
        "expected LF-normalized lines: {out:?}"
    );
    assert!(
        !out.contains('\r'),
        "output should not contain CR characters: {out:?}"
    );
}
