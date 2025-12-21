mod common;
use insta::assert_snapshot;

#[test]
fn snapshot_grep_per_slot_line_cap() {
    let out = common::run_cli(
        &[
            "--no-color",
            "--no-sort",
            "--grep",
            "return",
            "--grep-show",
            "all",
            "-n",
            "1",
            "tests/fixtures/code/sample.py",
            "tests/fixtures/code/sample.ts",
        ],
        None,
    );
    let out = out.stdout;
    assert_snapshot!(
        "grep_per_slot_line_cap",
        common::normalize_snapshot_paths(&out)
    );
}

#[test]
fn snapshot_counted_headers_tiny_line_cap() {
    let out = common::run_cli(
        &[
            "--no-color",
            "--no-sort",
            "-H",
            "-n",
            "1",
            "tests/fixtures/mixed_headers/a.json",
            "tests/fixtures/mixed_headers/b.yaml",
            "tests/fixtures/mixed_headers/c.txt",
        ],
        None,
    );
    let out = out.stdout;
    assert_snapshot!(
        "counted_headers_tiny_line_cap",
        common::normalize_snapshot_paths(&out)
    );
}

#[test]
fn snapshot_tree_per_slot_line_cap() {
    let out = common::run_cli(
        &[
            "--no-color",
            "--tree",
            "--no-sort",
            "tests/fixtures/tree_per_slot/a.txt",
            "tests/fixtures/tree_per_slot/b.txt",
            "-n",
            "1",
        ],
        None,
    );
    let out = out.stdout;
    assert_snapshot!(
        "tree_per_slot_line_cap",
        common::normalize_snapshot_paths(&out)
    );
}

#[test]
fn snapshot_tree_per_slot_varied_line_cap() {
    let out = common::run_cli(
        &[
            "--no-color",
            "--tree",
            "--no-sort",
            "tests/fixtures/tree_per_slot_varied/a.txt",
            "tests/fixtures/tree_per_slot_varied/b.txt",
            "tests/fixtures/tree_per_slot_varied/c.txt",
            "tests/fixtures/tree_per_slot_varied/d.txt",
            "-n",
            "3",
        ],
        None,
    );
    let out = out.stdout;
    assert_snapshot!(
        "tree_per_slot_varied_line_cap",
        common::normalize_snapshot_paths(&out)
    );
}

#[test]
fn snapshot_multibyte_chars_and_bytes_per_slot() {
    let out = common::run_cli(
        &[
            "--no-color",
            "--no-sort",
            "--chars",
            "6",
            "--global-bytes",
            "12",
            "tests/fixtures/bytes_chars/emoji.json",
            "tests/fixtures/bytes_chars/long.txt",
        ],
        None,
    );
    let out = out.stdout;
    assert_snapshot!(
        "multibyte_chars_and_bytes_per_slot",
        common::normalize_snapshot_paths(&out)
    );
}

#[test]
fn snapshot_multibyte_chars_tighter_than_bytes() {
    let out = common::run_cli(
        &[
            "--no-color",
            "--no-sort",
            "--tree",
            "--chars",
            "12",
            "--global-bytes",
            "100",
            "tests/fixtures/chars_vs_bytes/emoji.txt",
            "tests/fixtures/chars_vs_bytes/ascii.txt",
        ],
        None,
    );
    let out = out.stdout;
    assert_snapshot!(
        "multibyte_chars_tighter_than_bytes",
        common::normalize_snapshot_paths(&out)
    );
}
