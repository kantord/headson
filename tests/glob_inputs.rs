#[path = "../test_support/mod.rs"]
mod util;

use std::fs;
use std::path::Path;

fn write_json(path: &Path, body: &str) {
    fs::write(path, body).expect("write json");
}

#[test]
fn glob_expands_recursively_and_respects_gitignore() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();
    fs::create_dir_all(root.join("src/nested")).expect("mkdirs");

    write_json(root.join("src/keep.json").as_path(), r#"{"keep": true}"#);
    write_json(
        root.join("src/nested/also_keep.json").as_path(),
        r#"{"nested": true}"#,
    );
    write_json(
        root.join("src/ignored.json").as_path(),
        r#"{"ignore": true}"#,
    );
    write_json(
        root.join("src/nested/ignored.json").as_path(),
        r#"{"ignore_nested": true}"#,
    );
    fs::write(root.join(".gitignore"), "ignored.json\n")
        .expect("write gitignore");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .current_dir(root)
        .args([
            "--no-color",
            "--no-sort",
            "-c",
            "1000",
            "-g",
            "src/**/*.json",
        ])
        .assert();

    let ok = assert.get_output().status.success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(ok, "glob run should succeed: {out}");
    let keep_header =
        format!("==> {} <==", Path::new("src").join("keep.json").display());
    let nested_header = format!(
        "==> {} <==",
        Path::new("src")
            .join("nested")
            .join("also_keep.json")
            .display()
    );
    let ignored_header = format!(
        "==> {} <==",
        Path::new("src").join("ignored.json").display()
    );
    let ignored_nested_header = format!(
        "==> {} <==",
        Path::new("src")
            .join("nested")
            .join("ignored.json")
            .display()
    );
    assert!(
        out.contains(&keep_header),
        "expected keep.json to be included: {out}"
    );
    assert!(
        out.contains(&nested_header),
        "expected nested file to be included: {out}"
    );
    assert!(
        !out.contains(&ignored_header)
            && !out.contains(&ignored_nested_header),
        "gitignored files should be skipped: {out}"
    );
}

#[test]
fn glob_no_sort_preserves_pattern_order() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    write_json(root.join("a.json").as_path(), r#"{"a": 1}"#);
    write_json(root.join("b.json").as_path(), r#"{"b": 2}"#);

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .current_dir(root)
        .args([
            "--no-color",
            "--no-sort",
            "-c",
            "1000",
            "-g",
            "b*.json",
            "-g",
            "a*.json",
        ])
        .assert();

    let ok = assert.get_output().status.success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(ok, "glob run should succeed: {out}");
    let header_a = format!("==> {} <==", Path::new("a.json").display());
    let header_b = format!("==> {} <==", Path::new("b.json").display());
    let pos_a = out
        .find(&header_a)
        .expect("should include a.json in output");
    let pos_b = out
        .find(&header_b)
        .expect("should include b.json in output");
    assert!(
        pos_b < pos_a,
        "glob expansion should follow user-provided pattern order with --no-sort: {out}"
    );
}

#[test]
fn glob_inputs_deduplicate_overlaps_and_explicit_paths() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();
    write_json(root.join("one.json").as_path(), r#"{"a": 1}"#);
    write_json(root.join("two.json").as_path(), r#"{"b": 2}"#);

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .current_dir(root)
        .args([
            "--no-color",
            "--no-sort",
            "-c",
            "1000",
            "-g",
            "*.json",
            "one.json",
        ])
        .assert();

    let ok = assert.get_output().status.success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(ok, "glob + explicit run should succeed: {out}");

    let header_one = format!("==> {} <==", Path::new("one.json").display());
    let header_two = format!("==> {} <==", Path::new("two.json").display());
    let one_count = out.matches(&header_one).count();
    let two_count = out.matches(&header_two).count();
    assert_eq!(
        1, one_count,
        "one.json should appear once even when matched twice: {out}"
    );
    assert_eq!(
        1, two_count,
        "two.json should appear once from the glob: {out}"
    );
}

#[test]
fn glob_with_no_matches_emits_notice_instead_of_blocking_on_stdin() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .current_dir(root)
        .args(["--no-color", "-g", "*.json"])
        .assert();

    let ok = assert.get_output().status.success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(ok, "glob with no matches should still succeed: {err}");
    assert_eq!(out, "\n", "stdout should stay empty: {out:?}");
    assert!(
        err.contains("No files matched"),
        "expected notice about unmatched globs: {err:?}"
    );
}
