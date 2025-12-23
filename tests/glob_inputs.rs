mod common;

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

    let out = common::run_cli_in_dir(
        root,
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "1000",
            "-g",
            "src/**/*.json",
        ],
        None,
    );

    let out = out.stdout;
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
fn recursive_expands_recursively_and_respects_gitignore() {
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

    let out = common::run_cli_in_dir(
        root,
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "1000",
            "--recursive",
            "src",
        ],
        None,
    );

    let out = out.stdout;
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
fn recursive_respects_nested_gitignore_negation() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();
    fs::create_dir_all(root.join("src/nested")).expect("mkdirs");

    fs::write(root.join(".gitignore"), "*.log\n").expect("write gitignore");
    fs::write(root.join("src/nested/.gitignore"), "!keep.log\n")
        .expect("write nested gitignore");

    fs::write(root.join("src/ignored.log"), "ignore")
        .expect("write ignored log");
    fs::write(root.join("src/nested/keep.log"), "keep")
        .expect("write keep log");

    let out = common::run_cli_in_dir(
        root,
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "1000",
            "--recursive",
            "src",
        ],
        None,
    );

    let out = out.stdout;
    assert!(
        !out.contains("ignore"),
        "expected ignored.log to be excluded: {out}"
    );
    assert!(
        out.contains("keep"),
        "expected keep.log to be re-included: {out}"
    );
}

#[test]
fn glob_rejects_negated_patterns() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    let raw = common::build_cmd_in_dir(
        root,
        &["--no-color", "-g", "!src/vendor/**"],
        None,
    )
    .assert()
    .failure()
    .get_output()
    .clone();
    let stderr = String::from_utf8_lossy(&raw.stderr);
    assert!(
        stderr.contains("negated glob patterns are not supported"),
        "expected negated glob error, got: {stderr}"
    );
}

#[test]
fn recursive_respects_ignore_and_rgignore() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();
    fs::create_dir_all(root.join("src")).expect("mkdirs");

    fs::write(root.join(".ignore"), "*.tmp\n").expect("write .ignore");
    fs::write(root.join(".rgignore"), "skip.log\n").expect("write .rgignore");

    fs::write(root.join("src/skip.log"), "skip").expect("write ignored log");
    fs::write(root.join("src/ignored.tmp"), "ignore")
        .expect("write ignored tmp");
    fs::write(root.join("src/keep.txt"), "keep").expect("write keep file");

    let out = common::run_cli_in_dir(
        root,
        &["--no-color", "-c", "1000", "--recursive", "src"],
        None,
    );

    let out = out.stdout;
    assert!(
        out.contains("keep"),
        "expected keep.txt to be included: {out}"
    );
    assert!(
        !out.contains("ignore"),
        "expected ignored tmp to be excluded: {out}"
    );
    assert!(
        !out.contains("skip"),
        "expected ignored log to be excluded: {out}"
    );
}

#[test]
fn recursive_outside_cwd_ignores_only_target_tree() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let cwd = tmp.path();
    fs::write(cwd.join(".gitignore"), "*.json\n").expect("write gitignore");

    let other = tempfile::tempdir().expect("tmpdir");
    let other_root = other.path();
    write_json(other_root.join("keep.json").as_path(), r#"{"ok": true}"#);

    let out = common::run_cli_in_dir(
        cwd,
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "1000",
            "--recursive",
            other_root.to_str().expect("other path"),
        ],
        None,
    );

    let out = out.stdout;
    assert!(
        out.contains("\"ok\": true"),
        "expected keep.json from outside cwd to be included: {out}"
    );
}

#[test]
fn glob_no_sort_preserves_pattern_order() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    write_json(root.join("a.json").as_path(), r#"{"a": 1}"#);
    write_json(root.join("b.json").as_path(), r#"{"b": 2}"#);

    let out = common::run_cli_in_dir(
        root,
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "1000",
            "-g",
            "b*.json",
            "-g",
            "a*.json",
        ],
        None,
    );

    let out = out.stdout;
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
fn recursive_rejects_file_inputs() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();
    write_json(root.join("one.json").as_path(), r#"{"a": 1}"#);

    let raw = common::build_cmd_in_dir(
        root,
        &["--no-color", "--recursive", "one.json"],
        None,
    )
    .assert()
    .failure()
    .get_output()
    .clone();
    let stderr = String::from_utf8_lossy(&raw.stderr);
    assert!(
        stderr.contains("requires directory inputs"),
        "expected directory-only error, got: {stderr}"
    );
}

#[test]
fn recursive_rejects_mixed_inputs() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();
    fs::create_dir_all(root.join("src")).expect("mkdirs");
    write_json(root.join("one.json").as_path(), r#"{"a": 1}"#);

    let raw = common::build_cmd_in_dir(
        root,
        &["--no-color", "--recursive", "src", "one.json"],
        None,
    )
    .assert()
    .failure()
    .get_output()
    .clone();
    let stderr = String::from_utf8_lossy(&raw.stderr);
    assert!(
        stderr.contains("requires directory inputs"),
        "expected directory-only error, got: {stderr}"
    );
}

#[test]
fn recursive_rejects_missing_inputs() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    let raw = common::build_cmd_in_dir(
        root,
        &["--no-color", "--recursive", "does-not-exist"],
        None,
    )
    .assert()
    .failure()
    .get_output()
    .clone();
    let stderr = String::from_utf8_lossy(&raw.stderr);
    assert!(
        stderr.contains("failed to read input path"),
        "expected missing path error, got: {stderr}"
    );
}

#[test]
fn recursive_deduplicates_overlapping_dirs() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();
    fs::create_dir_all(root.join("src/nested")).expect("mkdirs");
    write_json(root.join("src/one.json").as_path(), r#"{"a": 1}"#);
    write_json(root.join("src/nested/two.json").as_path(), r#"{"b": 2}"#);

    let out = common::run_cli_in_dir(
        root,
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "1000",
            "--recursive",
            "src",
            "src/nested",
        ],
        None,
    );

    let out = out.stdout;
    let header_two = format!(
        "==> {} <==",
        Path::new("src").join("nested").join("two.json").display()
    );
    let count_two = out.matches(&header_two).count();
    assert_eq!(
        1, count_two,
        "nested file should appear once even when overlapped: {out}"
    );
}

#[test]
fn glob_inputs_deduplicate_overlaps_and_explicit_paths() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();
    write_json(root.join("one.json").as_path(), r#"{"a": 1}"#);
    write_json(root.join("two.json").as_path(), r#"{"b": 2}"#);

    let out = common::run_cli_in_dir(
        root,
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "1000",
            "-g",
            "*.json",
            "one.json",
        ],
        None,
    );

    let out = out.stdout;

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
fn recursive_rejects_stdin() {
    let raw = common::build_cmd(&["--recursive"], Some(b"{}"))
        .assert()
        .failure()
        .get_output()
        .clone();
    let stderr = String::from_utf8_lossy(&raw.stderr);
    assert!(
        stderr.contains("stdin"),
        "expected stdin rejection, got: {stderr}"
    );
}

#[test]
fn glob_with_no_matches_emits_notice_instead_of_blocking_on_stdin() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    let output =
        common::run_cli_in_dir(root, &["--no-color", "-g", "*.json"], None);

    let out = output.stdout;
    let err = output.stderr;
    assert_eq!(out, "\n", "stdout should stay empty: {out:?}");
    assert!(
        err.contains("No files matched"),
        "expected notice about unmatched globs: {err:?}"
    );
}

#[test]
fn recursive_with_no_matches_emits_notice() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();
    fs::create_dir_all(root.join("src")).expect("mkdirs");

    let output = common::run_cli_in_dir(
        root,
        &["--no-color", "--recursive", "src"],
        None,
    );

    let out = output.stdout;
    let err = output.stderr;
    assert_eq!(out, "\n", "stdout should stay empty: {out:?}");
    assert!(
        err.contains("No files matched"),
        "expected notice about unmatched recursive inputs: {err:?}"
    );
}

#[test]
fn tree_glob_with_no_matches_still_emits_notice() {
    // Regression test: --tree used to bail out early when glob expansion found
    // no files, instead of emitting the usual notice and exiting successfully.
    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    let output = common::run_cli_in_dir(
        root,
        &["--no-color", "--tree", "-g", "*.json"],
        None,
    );

    let out = output.stdout;
    let err = output.stderr;
    assert_eq!(out, "\n", "stdout should stay empty: {out:?}");
    assert!(
        err.contains("No files matched"),
        "expected notice about unmatched globs: {err:?}"
    );
}
