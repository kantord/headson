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
