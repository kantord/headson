use assert_cmd::cargo::cargo_bin_cmd;
use std::{fs, path::Path};
use tempfile::tempdir;

fn write_file(path: &Path, contents: &str) {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).expect("mkdirs");
    }
    fs::write(path, contents).expect("write");
}

#[test]
fn tree_renders_nested_files_with_code_gutters() {
    let dir = tempdir().expect("tmp");
    write_file(
        &dir.path().join("src/main.rs"),
        "fn main() {\n    println!(\"hi\");\n}\n",
    );
    write_file(
        &dir.path().join("src/ingest/fileset.rs"),
        "pub fn merge_filesets() {}\nfn helper() {}\n",
    );
    write_file(&dir.path().join("data/users.json"), r#"{"users":[1,2,3]}"#);
    write_file(&dir.path().join("README.md"), "headson tree preview\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--tree",
            "--no-sort",
            "-c",
            "400",
            "src/main.rs",
            "src/ingest/fileset.rs",
            "data/users.json",
            "README.md",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = concat!(
        ".\n",
        "├─ src/\n",
        "│ ├─ main.rs\n",
        "│ │ 1: fn main() {\n",
        "│ │ 2:     println!(\"hi\");\n",
        "│ │ 3: }\n",
        "│ ├─ ingest/fileset.rs\n",
        "│ │ 1: pub fn merge_filesets() {}\n",
        "│ │ 2: fn helper() {}\n",
        "├─ data/users.json\n",
        "│ {\n",
        "│   \"users\": [\n",
        "│     1,\n",
        "│     2,\n",
        "│     3\n",
        "│   ]\n",
        "│ }\n",
        "├─ README.md\n",
        "│ 1: headson tree preview\n",
        "\n",
    );
    assert_eq!(stdout.as_ref(), expected);
}

#[test]
fn tree_emits_omission_marker_under_tight_budget() {
    let dir = tempdir().expect("tmp");
    write_file(
        &dir.path().join("src/lib.rs"),
        "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\n",
    );

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--tree",
            "--no-sort",
            "--bytes",
            "60",
            "src/lib.rs",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = concat!(
        ".\n",
        "├─ src/lib.rs\n",
        "│ 1: fn a() {}\n",
        "│ 3: fn c() {}\n",
        "\n",
    );
    assert_eq!(stdout.as_ref(), expected);
    assert!(
        !stdout.contains("fn e"),
        "budget should truncate file content in tree mode"
    );
}

#[test]
fn tree_renders_duplicate_basenames_in_distinct_dirs() {
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("a/foo.rs"), "fn a() {}\n");
    write_file(&dir.path().join("b/foo.rs"), "fn b() {}\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--tree", "--no-sort", "a/foo.rs", "b/foo.rs"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("├─ a/foo.rs") && stdout.contains("├─ b/foo.rs"),
        "tree view should show both files with tee branches: {stdout}"
    );
    assert!(
        stdout.contains("a/foo.rs") && stdout.contains("b/foo.rs"),
        "paths should stay disambiguated even when basenames repeat: {stdout}"
    );
}

#[test]
fn tree_keeps_branch_connectors_for_last_child_lines() {
    let dir = tempdir().expect("tmp");
    write_file(
        &dir.path().join("dir/only.rs"),
        "fn main() {}\nlet _x = 1;\n",
    );

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--tree", "--no-sort", "dir/only.rs"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("├─ dir/only.rs"),
        "single child should still render with a tee branch for vertical continuity: {stdout}"
    );
    assert!(
        stdout.contains("│ 1: fn main() {}"),
        "line gutters should remain aligned under the tree prefix: {stdout}"
    );
}

#[test]
fn tree_colorizes_pipes_and_names_when_color_enabled() {
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("a.rs"), "fn a() {}\n");
    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .env("FORCE_COLOR", "1")
        .args(["--tree", "--no-sort", "a.rs"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\u{001b}[90m├─ \u{001b}[0m")
            || stdout.contains("\u{001b}[90m├─\u{001b}[0m"),
        "branch pipes should be colored when color is enabled: {stdout:?}"
    );
    assert!(
        stdout.contains("\u{001b}[1;34ma.rs\u{001b}[0m"),
        "file name should be colored like keys: {stdout:?}"
    );
}

#[test]
fn tree_remains_plain_when_color_disabled() {
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("b.rs"), "fn b() {}\n");
    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--tree", "--no-sort", "b.rs"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains("\u{001b}["),
        "no ANSI escapes should appear when color is disabled: {stdout:?}"
    );
}

#[test]
fn tree_with_grep_keeps_match_highlights_and_colored_pipes() {
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("c.json"), r#"{"k":"needle","x":"other"}"#);
    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .env("FORCE_COLOR", "1")
        .args(["--tree", "--grep", "needle", "--no-sort", "c.json"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\u{001b}[31mneedle\u{001b}[39m"),
        "grep highlight should still color the match: {stdout:?}"
    );
    assert!(
        !stdout.contains("\u{001b}[90m├─"),
        "pipes should follow grep highlight-only rules (no syntax colors in tree chrome): {stdout:?}"
    );
}

#[test]
fn tree_reports_omitted_files_when_budget_drops_them() {
    let dir = tempdir().expect("tmp");
    for name in ["a", "b", "c", "d", "e"] {
        write_file(&dir.path().join(format!("{name}.txt")), "line\n");
    }

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--tree",
            "--no-sort",
            "-N",
            "4",
            "a.txt",
            "b.txt",
            "c.txt",
            "d.txt",
            "e.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("… 4 more items"),
        "when the budget is too small for most files, tree mode should report how many items were omitted: {stdout}"
    );
}
