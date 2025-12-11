use assert_cmd::cargo::cargo_bin_cmd;
use insta::assert_snapshot;
use std::collections::HashSet;
use std::{fs, path::Path};
use tempfile::tempdir;

fn write_file(path: &Path, contents: &str) {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).expect("mkdirs");
    }
    fs::write(path, contents).expect("write");
}

fn tree_cfg() -> headson::RenderConfig {
    headson::RenderConfig {
        template: headson::OutputTemplate::Auto,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: headson::ColorMode::Off,
        color_enabled: false,
        style: headson::Style::Default,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        fileset_tree: true,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    }
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
        "│ 2: fn b() {}\n",
        "│ 3: fn c() {}\n",
        "│ 5: fn e() {}\n",
        "\n",
    );
    assert_eq!(stdout.as_ref(), expected);
    assert!(
        !stdout.contains("fn d"),
        "budget should truncate file content in tree mode"
    );
}

#[test]
fn tree_counted_headers_with_per_file_cap_completes() {
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("a.txt"), "one\n");
    write_file(&dir.path().join("b.txt"), "two\nthree\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--tree",
            "--no-sort",
            "-H",
            "-n",
            "1",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("a.txt") && stdout.contains("b.txt"),
        "tree output should mention both files even when headers are counted: {stdout}"
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
fn tree_respects_per_file_line_budget() {
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("a.txt"), "a1\na2\na3\n");
    write_file(&dir.path().join("b.txt"), "b1\nb2\nb3\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--tree",
            "-n",
            "2",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("├─ a.txt"),
        "first file should appear in tree: {stdout}"
    );
    assert!(
        stdout.contains("├─ b.txt"),
        "second file should appear in tree: {stdout}"
    );
    assert!(
        stdout.contains("a1") && stdout.contains("b1"),
        "each file should keep head content under per-file cap: {stdout}"
    );
    assert!(
        !stdout.contains("a2") && !stdout.contains("b2"),
        "content beyond the per-file line budget should be omitted: {stdout}"
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
        stdout.contains("\u{001b}[90m├─ \u{001b}[0m")
            || stdout.contains("\u{001b}[90m├─\u{001b}[0m"),
        "pipes should remain colored even when grep is in highlight-only mode: {stdout:?}"
    );
}

#[test]
fn tree_with_grep_reports_non_matching_files() {
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("a.txt"), "miss\n");
    write_file(&dir.path().join("b.txt"), "miss\n");
    write_file(&dir.path().join("c.txt"), "hit\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--tree",
            "--no-sort",
            "--grep",
            "hit",
            "a.txt",
            "b.txt",
            "c.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let summary_expected =
        concat!(".\n", "├─ c.txt\n", "│ hit\n", "├─ … 2 more items\n", "\n",);
    if stdout.as_ref() != summary_expected {
        // Allow per-file omissions when the renderer keeps file entries but elides bodies.
        assert!(
            stdout.contains("a.txt\n│ …\n")
                && stdout.contains("b.txt\n│ …\n")
                && stdout.contains("c.txt\n│ hit\n"),
            "tree mode should either summarize non-matching files once or mark each file as omitted: {stdout}"
        );
    }
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
        stdout.contains("… 2 more items"),
        "when the budget is too small for most files, tree mode should report how many items were omitted: {stdout}"
    );
}

#[test]
fn tree_reports_omissions_when_every_file_is_dropped() {
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("dir/a.txt"), "line\n");
    write_file(&dir.path().join("dir/b.txt"), "line\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--tree",
            "--no-sort",
            "-N",
            "0",
            "dir/a.txt",
            "dir/b.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = concat!(".\n", "└─ dir/\n", "│ ├─ … 2 more items\n", "\n",);
    assert_eq!(
        stdout.as_ref(),
        expected,
        "when all files are pruned, omission counts should still render under their folder"
    );
}

#[test]
fn tree_respects_line_budget_by_dropping_all_content() {
    // With a line budget of 1, no file content should be rendered; instead only
    // an omission marker should appear (tree scaffolding is treated as header-like).
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("a.txt"), "line\n");
    write_file(&dir.path().join("b.txt"), "line\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--tree",
            "--no-sort",
            "-N",
            "1",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = concat!(".\n", "├─ … 2 more items\n", "\n",);
    assert_eq!(
        stdout.as_ref(),
        expected,
        "line budget should drop all file content and surface a single omission marker"
    );
}

#[test]
fn tree_budget_omissions_append_after_kept_files() {
    // With a tight byte budget, keep the first file (truncated) and ensure the
    // root-level omission marker for the remaining files appears at the end.
    let dir = tempdir().expect("tmp");
    write_file(
        &dir.path().join("big1.txt"),
        "aaaa\naaaa\naaaa\naaaa\naaaa\n",
    );
    write_file(&dir.path().join("small1.txt"), "bbbb\n");
    write_file(&dir.path().join("small2.txt"), "cccc\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--tree",
            "--no-sort",
            "-N",
            "3",
            "big1.txt",
            "small1.txt",
            "small2.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = concat!(
        ".\n",
        "├─ big1.txt\n",
        "│ aaaa\n",
        "│ …\n",
        "├─ … 2 more items\n",
        "\n",
    );
    assert_eq!(
        stdout.as_ref(),
        expected,
        "root-level omission marker should be merged and appear after kept content"
    );
}

#[test]
fn tree_keeps_identical_files_under_tight_line_budget() {
    // Regression: cross-file duplicate penalties should not starve identical files.
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("a.py"), "def foo():\n    pass\n");
    write_file(&dir.path().join("b.py"), "def foo():\n    pass\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--tree",
            "--no-sort",
            "-n",
            "2",
            "a.py",
            "b.py",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("├─ a.py") && stdout.contains("│ 1: def foo():"),
        "first identical file should render its first line under tight line budget: {stdout}"
    );
    assert!(
        stdout.contains("├─ b.py") && stdout.contains("│ 1: def foo():"),
        "second identical file should also render content despite duplicate lines: {stdout}"
    );
}

#[test]
fn tree_cli_snapshot_budgeted_root_omission() {
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
    assert_snapshot!(
        "tree_cli_snapshot_budgeted_root_omission",
        stdout.as_ref()
    );
}

#[test]
fn tree_cli_color_snapshot_with_grep() {
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("main.rs"), "fn main(){}\n");
    write_file(&dir.path().join("lib.rs"), "fn helper(){}\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .env("FORCE_COLOR", "1")
        .args(["--tree", "--no-sort", "--grep", "main", "main.rs", "lib.rs"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_snapshot!("tree_cli_color_snapshot_with_grep", stdout.as_ref());
}

#[test]
fn tree_cli_snapshot_nested_folder_omission() {
    let dir = tempdir().expect("tmp");
    write_file(&dir.path().join("keep/root.rs"), "fn keep(){}\n");
    write_file(&dir.path().join("omit/deep/file.rs"), "fn drop(){}\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--tree",
            "--no-sort",
            "-N",
            "3",
            "keep/root.rs",
            "omit/deep/file.rs",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_snapshot!(
        "tree_cli_snapshot_nested_folder_omission",
        stdout.as_ref()
    );
}

#[test]
fn tree_omitted_folders_render_in_input_order() {
    // With only omission markers left, the tree should stay deterministic and
    // respect the original fileset order instead of hash-map iteration.
    let render_once = || {
        let files = ["a", "b", "c", "d", "e"]
            .into_iter()
            .map(|name| headson::FilesetInput {
                name: format!("{name}/file.txt"),
                bytes: b"line\n".to_vec(),
                kind: headson::FilesetInputKind::Text { atomic_lines: true },
            })
            .collect();
        let cfg = tree_cfg();
        let prio = headson::PriorityConfig {
            max_string_graphemes: 500,
            array_max_items: 8,
            prefer_tail_arrays: false,
            array_bias: headson::ArrayBias::HeadMidTail,
            array_sampler: headson::ArraySamplerStrategy::Default,
            line_budget_only: true,
        };
        let grep_cfg = headson::GrepConfig::default();
        let budgets = headson::Budgets {
            global: Some(headson::Budget {
                kind: headson::BudgetKind::Lines,
                cap: 0,
            }),
            per_slot: Some(headson::Budget {
                kind: headson::BudgetKind::Lines,
                cap: 0,
            }),
        };
        headson::headson(
            headson::InputKind::Fileset(files),
            &cfg,
            &prio,
            &grep_cfg,
            budgets,
        )
        .expect("render tree")
    };

    // Run multiple times to flush out nondeterministic ordering.
    let mut variants = HashSet::new();
    for _ in 0..8 {
        variants.insert(render_once());
    }
    assert_eq!(
        variants.len(),
        1,
        "tree output with only omissions should be stable; saw variants: {variants:?}"
    );
}
