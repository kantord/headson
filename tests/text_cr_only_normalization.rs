mod common;

#[test]
fn text_normalizes_bare_cr_to_lf() {
    // Provide only '\r' newlines; expect LF normalization.
    let input = b"a\rb\rc\r".to_vec();
    let out = common::run_cli(
        ["--no-color", "-i", "text", "-f", "text", "-c", "1000"].as_ref(),
        Some(&input),
    );
    let out = out.stdout;
    assert!(
        out.contains("a\nb\nc\n"),
        "expected LF-normalized lines: {out:?}"
    );
    assert!(
        !out.contains('\r'),
        "output should not contain CR characters: {out:?}"
    );
}
