mod common;

use std::fs;

fn write_file(path: &std::path::Path, body: &str) {
    fs::write(path, body).expect("write file");
}

#[test]
fn fileset_parse_error_renders_empty_entry() {
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
        "expected bad.json header to render: {}",
        out.stdout
    );
    assert!(
        !out.stdout.contains("==> bad.json <==\n{}"),
        "expected bad.json body to be empty: {}",
        out.stdout
    );
    assert!(
        !out.stdout.contains("INVALID_JSON_"),
        "expected bad.json content to be omitted: {}",
        out.stdout
    );
    assert!(
        out.stderr.contains("bad.json")
            && out.stderr.contains("Failed to parse"),
        "expected stderr notice about fallback: {}",
        out.stderr
    );
}

#[test]
fn fileset_tree_parse_error_renders_empty_entry() {
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
        "expected bad.json to remain in tree: {}",
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
