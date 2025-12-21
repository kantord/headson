mod common;

#[test]
fn text_stdin_snapshot() {
    let input = b"a\r\nb\r\nc\r\n".to_vec();
    let out = common::run_cli(
        &["--no-color", "-i", "text", "-f", "text"],
        Some(&input),
    );
    let out = common::normalize_trailing_newline(&out.stdout);
    insta::assert_snapshot!(out);
}

#[test]
fn fileset_text_files_snapshot() {
    let dir = tempfile::tempdir().expect("tmpdir");
    std::fs::write(dir.path().join("a.txt"), b"one\ntwo\n").unwrap();
    std::fs::write(dir.path().join("b.log"), b"alpha\nbeta\n").unwrap();

    let out = common::run_cli_in_dir(
        dir.path(),
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "10000",
            "-f",
            "auto",
            "a.txt",
            "b.log",
        ],
        None,
    );
    let out = common::normalize_trailing_newline(&out.stdout);
    insta::assert_snapshot!(out);
}
