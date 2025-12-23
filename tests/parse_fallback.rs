mod common;

use std::fs;

fn write_file(path: &std::path::Path, body: &str) {
    fs::write(path, body).expect("write file");
}

#[test]
fn fileset_parse_error_falls_back_to_text() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    write_file(root.join("good.json").as_path(), r#"{"ok": true}"#);
    write_file(root.join("bad.json").as_path(), "INVALID_JSON_{");

    let out = common::run_cli_in_dir(
        root,
        &["--no-color", "-c", "1000", "good.json", "bad.json"],
        None,
    );

    assert!(
        out.stdout.contains("==> bad.json <=="),
        "expected bad.json to be included: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("INVALID_JSON_"),
        "expected bad.json to fall back to text: {}",
        out.stdout
    );
    assert!(
        out.stderr.contains("bad.json")
            && out.stderr.contains("falling back to text"),
        "expected stderr notice about fallback: {}",
        out.stderr
    );
}

#[test]
fn fileset_tree_parse_error_falls_back_to_text() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    write_file(root.join("good.json").as_path(), r#"{"ok": true}"#);
    write_file(root.join("bad.json").as_path(), "INVALID_JSON_{");

    let out = common::run_cli_in_dir(
        root,
        &[
            "--no-color",
            "--tree",
            "-c",
            "1000",
            "good.json",
            "bad.json",
        ],
        None,
    );

    assert!(
        out.stdout.contains("bad.json"),
        "expected bad.json to be present in tree: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("INVALID_JSON_"),
        "expected bad.json to fall back to text in tree: {}",
        out.stdout
    );
}

#[test]
fn single_file_parse_error_still_fails() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();
    write_file(root.join("bad.json").as_path(), "INVALID_JSON_{");

    let out = common::run_cli_in_dir_expect_fail(
        root,
        &["--no-color", "bad.json"],
        None,
        None,
    );
    assert!(
        out.stderr.contains("bad.json"),
        "expected parse failure to mention filename: {}",
        out.stderr
    );
}
