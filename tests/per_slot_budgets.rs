use assert_cmd::cargo::cargo_bin_cmd;
use std::collections::HashMap;
use std::fs;
use tempfile::{TempDir, tempdir};

fn write_file(dir: &TempDir, name: &str, contents: &str) {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdirs");
    }
    fs::write(path, contents).expect("write");
}

#[test]
fn per_file_line_budget_respected() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\na3\n");
    write_file(&dir, "b.txt", "b1\nb2\nb3\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "1", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("==> a.txt <=="),
        "first file header should render under per-file cap: {stdout}"
    );
    assert!(
        stdout.contains('…'),
        "omission marker should indicate body elided under tight per-file cap: {stdout}"
    );
    assert!(
        stdout.contains("==> b.txt <=="),
        "second file header should still render under per-file line cap: {stdout}"
    );
}

#[test]
fn per_file_line_budget_keeps_context_with_strong_grep() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "alpha\nneedle1\nbeta\nneedle2\ngamma\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "needle",
            "--grep-show",
            "all",
            "-n",
            "3",
            "a.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("alpha")
            && stdout.contains("beta")
            && stdout.contains("gamma"),
        "per-file line cap should apply to non-matching context when grep makes matches free: {stdout}"
    );
    assert!(
        stdout.contains("needle1") && stdout.contains("needle2"),
        "strong grep should still force matches even if they exceed the cap: {stdout}"
    );
    assert!(
        !stdout.contains('…'),
        "all non-matching context should fit under the per-file cap once matches are free: {stdout}"
    );
}

#[test]
fn per_file_line_budget_counts_headers() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-H", "-n", "1", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains("\na1\n") && !stdout.contains("\nb1\n"),
        "content should be skipped when header consumes the per-file line budget: {stdout}"
    );
}

#[test]
fn per_file_line_budget_one_with_counted_headers_emits_no_ellipsis() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-H", "-n", "1", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains('…'),
        "per-file line budget of one with counted headers should not emit an extra omission line: {stdout}"
    );
}

#[test]
fn per_file_line_budget_leaves_room_for_minimal_bodies() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.json", "{}\n");
    write_file(&dir, "b.yaml", "{}\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "-H",
            "-n",
            "2",
            "a.json",
            "b.yaml",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("==> a.json <==\n{}\n"),
        "JSON body should still render when header + body fit under per-file cap: {stdout}"
    );
    assert!(
        stdout.contains("==> b.yaml <==\n{}\n"),
        "YAML body should also render under the same per-file cap: {stdout}"
    );
}

#[test]
fn per_file_line_budget_zero_with_counted_headers_outputs_nothing() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-H", "-n", "0", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "per-file line budget of zero with counted headers should emit nothing: {stdout}"
    );
}

#[test]
fn per_file_line_budget_zero_without_headers_outputs_nothing() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "0", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "per-file line budget of zero without headers should emit nothing: {stdout}"
    );
}

#[test]
fn per_file_line_budget_zero_single_input_outputs_nothing() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "only.txt", "line1\nline2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "0", "only.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "single input should be fully suppressed when per-file line cap is zero: {stdout}"
    );
}

#[test]
fn per_file_byte_budget_prevents_starvation() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "long.txt", "abcdefg\nhijklmn\n");
    write_file(&dir, "short.txt", "x\ny\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--bytes",
            "8",
            "long.txt",
            "short.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("==> long.txt <=="),
        "long file should still appear even when truncated by per-file bytes: {stdout}"
    );
    assert!(
        stdout.contains("==> short.txt <=="),
        "second file should not be starved by the first: {stdout}"
    );
    // Body lines may be dropped under tight byte caps; headers must remain.
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "Test assembles and inspects output inline; splitting would add noise without clarity."
)]
fn per_file_line_budget_one_keeps_bodies_empty_when_headers_free() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.json", "{\"a\":1,\"b\":2,\"c\":3}\n");
    write_file(&dir, "b.json", "{\"x\":1,\"y\":2,\"z\":3}\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "1", "a.json", "b.json"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut current: Option<String> = None;
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("==> ") {
            if let Some((name, _)) = rest.split_once(" <==") {
                current = Some(name.to_string());
                counts.insert(name.to_string(), 0);
            }
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some(cur) = current.as_ref() {
            *counts.entry(cur.clone()).or_default() += 1;
        }
    }

    assert!(
        counts.values().all(|n| *n <= 1),
        "per-file line cap of 1 should not render multi-line bodies when headers are free: counts={counts:?}, out={stdout}"
    );
    assert!(
        counts.contains_key("a.json") && counts.contains_key("b.json"),
        "both files should still emit headers under per-file caps: {stdout}"
    );
}

#[test]
fn per_file_line_budget_respected_with_strong_grep() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.py", "def f():\n    return 1\n");
    write_file(&dir, "b.py", "def g():\n    pass\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "return",
            "--grep-show",
            "all",
            "-n",
            "1",
            "a.py",
            "b.py",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("return 1"),
        "matching file should include the return line even under per-file cap: {stdout}"
    );
    assert!(
        stdout.contains("==> b.py <=="),
        "non-matching file should still render its header with --grep-show: {stdout}"
    );
    assert!(
        !stdout.contains("pass"),
        "non-matching tail content should stay filtered under the per-file cap: {stdout}"
    );
}

#[test]
fn per_file_line_budget_respected_with_strong_grep_single_input() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "only.txt", "pre\nmatch\npost\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "match",
            "--grep-show",
            "all",
            "-n",
            "1",
            "only.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("match"),
        "strong grep should include the matching line: {stdout}"
    );
    assert!(
        !(stdout.contains("pre") && stdout.contains("post")),
        "per-file line cap of 1 should not allow both non-matching lines: {stdout}"
    );
    assert!(
        stdout.contains('…'),
        "omission marker should indicate truncated context under the per-file cap: {stdout}"
    );
}

#[test]
fn per_file_line_budget_respected_without_headers() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--no-header",
            "-n",
            "1",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains("==>"),
        "headers should be suppressed with --no-header: {stdout}"
    );
    assert!(
        stdout.lines().filter(|l| l.contains('…')).count() == 2,
        "each file should contribute a single omission line when headers are off: {stdout}"
    );
    assert!(
        !stdout.contains("a1")
            && !stdout.contains("a2")
            && !stdout.contains("b1")
            && !stdout.contains("b2"),
        "content beyond the per-file cap should be omitted: {stdout}"
    );
}

#[test]
fn per_file_zero_byte_or_char_budget_emits_nothing() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "only.txt", "data that should be hidden\n");

    for (flag, desc) in [("--bytes", "byte"), ("--chars", "char")] {
        let assert = cargo_bin_cmd!("hson")
            .current_dir(dir.path())
            .args(["--no-color", "--no-sort", flag, "0", "only.txt"])
            .assert()
            .success();

        let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
        assert!(
            stdout.trim().is_empty(),
            "per-file {desc} budget of zero should suppress all output: {stdout}"
        );
    }
}

#[test]
fn per_file_zero_byte_budget_emits_nothing_without_headers() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "only.txt", "hidden\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--no-header",
            "--bytes",
            "0",
            "only.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "per-file byte budget of zero with headers disabled should emit nothing: {stdout}"
    );
}

#[test]
fn per_file_zero_byte_budget_emits_nothing_with_counted_headers() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "only.txt", "hidden\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-H", "--bytes", "0", "only.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.trim().is_empty(),
        "per-file byte budget of zero with counted headers should emit nothing: {stdout}"
    );
}

#[test]
fn per_file_grep_multiple_hits_are_not_dropped() {
    let dir = tempdir().expect("tmp");
    write_file(
        &dir,
        "match.py",
        "def f():\n    return 1\n    return 2\n    return 3\n    x = 1\n",
    );
    write_file(&dir, "other.py", "def g():\n    pass\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "return",
            "--grep-show",
            "all",
            "-n",
            "1",
            "match.py",
            "other.py",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.matches("return").count() >= 3,
        "all matching lines should be kept even when per-file cap is tight: {stdout}"
    );
    assert!(
        stdout.contains("==> other.py <=="),
        "non-matching file should still surface under --grep-show all: {stdout}"
    );
    assert!(
        !stdout.contains("pass"),
        "non-matching content should remain filtered under per-file cap: {stdout}"
    );
}

#[test]
fn per_file_byte_budget_counts_headers() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "aaaaaa\nbbbbbb\n");
    write_file(&dir, "b.txt", "cccccc\ndddddd\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "--bytes", "12", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("==> a.txt <=="),
        "header should consume from the per-file byte budget but still render: {stdout}"
    );
    assert!(
        stdout.contains("==> b.txt <=="),
        "second header should also render under the per-file byte cap: {stdout}"
    );
    assert!(
        !stdout.contains("bbbbbb") && !stdout.contains("dddddd"),
        "tails should be truncated once the per-file byte budget is hit: {stdout}"
    );
}

#[test]
fn global_line_budget_does_not_override_per_slot_cap() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\na3\na4\n");
    write_file(&dir, "b.txt", "b1\nb2\nb3\nb4\n");
    write_file(&dir, "c.txt", "c1\nc2\nc3\nc4\n");
    write_file(&dir, "d.txt", "d1\nd2\nd3\nd4\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "-n",
            "2",
            "--global-lines",
            "50",
            "a.txt",
            "b.txt",
            "c.txt",
            "d.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for prefix in ["a", "b", "c", "d"] {
        assert!(
            stdout.contains(&format!("{prefix}1")),
            "each file should keep at least the first line under per-file cap: {stdout}"
        );
        assert!(
            !stdout.contains(&format!("{prefix}2")),
            "second line should be trimmed when omission marker must fit under the per-file cap: {stdout}"
        );
        assert!(
            stdout.contains("…"),
            "truncation marker should still appear while respecting per-file cap: {stdout}"
        );
    }
}

#[test]
fn per_file_line_budget_does_not_drop_small_files() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\n");
    write_file(&dir, "b.txt", "b1\nb2\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "5", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("a1\na2"),
        "small file should render fully when under per-file line budget: {stdout}"
    );
    assert!(
        stdout.contains("b1\nb2"),
        "second small file should also render fully: {stdout}"
    );
    assert!(
        !stdout.contains("…"),
        "no omission markers expected when under budget: {stdout}"
    );
}

#[test]
fn per_file_line_budget_keeps_string_prefix_in_line_only_mode() {
    let dir = tempdir().expect("tmp");
    write_file(
        &dir,
        "a.json",
        &format!("{{\"long\":\"{}\"}}", "A".repeat(400)),
    );
    write_file(
        &dir,
        "b.json",
        &format!("{{\"long\":\"{}\"}}", "B".repeat(400)),
    );

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "1", "a.json", "b.json"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("{ … }"),
        "line-only per-file cap should still show an elided object placeholder: {stdout}"
    );
}

#[test]
fn strong_grep_matches_do_not_exhaust_per_file_cap() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "log.txt", "pre\nmatch one\nmiddle\nmatch two\npost\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "match",
            "--grep-show",
            "all",
            "-n",
            "2",
            "log.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        ["pre", "middle", "post"]
            .iter()
            .any(|line| stdout.contains(line)),
        "some non-matching context should still appear when strong grep makes matches free: {stdout}"
    );
    assert!(
        stdout.contains("match one") && stdout.contains("match two"),
        "all matching lines should remain even if they exceed the per-file cap: {stdout}"
    );
    assert!(
        stdout.contains('…'),
        "an omission marker should signal trimmed context under the per-file cap: {stdout}"
    );
}

#[test]
fn strong_grep_does_not_starve_non_matching_context_in_filesets() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "match1\ncontext1\nmatch2\ncontext2\n");
    write_file(&dir, "b.txt", "other\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "--grep",
            "match",
            "--grep-show",
            "all",
            "-n",
            "1",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("match1") && stdout.contains("match2"),
        "strong grep should still surface all matching lines: {stdout}"
    );
    assert!(
        stdout.contains("context1") || stdout.contains("context2"),
        "per-file cap should still leave room for some non-matching context even when matches exceed the cap: {stdout}"
    );
    assert!(
        stdout.contains("==> b.txt <==") && stdout.contains("other"),
        "non-matching files should remain visible under --grep-show all: {stdout}"
    );
}

#[test]
fn counted_headers_respect_per_file_line_cap_under_strong_grep() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "pre\nmatch\nmid\npost\n");
    write_file(&dir, "b.txt", "other\n");

    let stdout = run_grep_with_counted_headers(&dir);
    assert_headers_and_match_present(&stdout);
    assert!(
        stdout.contains("\npre\n") || stdout.contains("\nmid\n"),
        "one non-matching line should remain under the per-file cap once the header is counted: {stdout}"
    );
    assert!(
        stdout.matches('\n').filter(|_| true).count() >= 3,
        "output should include header, match, and at least one context line under the cap: {stdout}"
    );
}

fn run_grep_with_counted_headers(dir: &TempDir) -> String {
    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args([
            "--no-color",
            "--no-sort",
            "-H",
            "-n",
            "2",
            "--grep",
            "match",
            "--grep-show",
            "all",
            "b.txt",
            "a.txt",
        ])
        .assert()
        .success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

fn assert_headers_and_match_present(stdout: &str) {
    assert!(
        stdout.contains("==> a.txt <=="),
        "header should render under counted per-file cap: {stdout}"
    );
    assert!(
        stdout.contains("==> b.txt <=="),
        "second header should also render under counted per-file cap: {stdout}"
    );
    assert!(
        stdout.contains("match"),
        "strong grep should still force the matching line even if it exceeds the cap: {stdout}"
    );
}

#[test]
fn per_file_line_budget_two_with_headers_free_still_truncates_body() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\na3\n");
    write_file(&dir, "b.txt", "b1\nb2\nb3\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-n", "2", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("==> a.txt <==\na1\n…\n"),
        "free headers with per-file line cap 2 should allow one body line then ellipsis for a.txt: {stdout}"
    );
    assert!(
        stdout.contains("==> b.txt <==\nb1\n…\n"),
        "free headers with per-file line cap 2 should allow one body line then ellipsis for b.txt: {stdout}"
    );
    assert!(
        !stdout.contains("\na2\n") && !stdout.contains("\nb2\n"),
        "second lines should be trimmed under the per-file cap: {stdout}"
    );
}

#[test]
fn per_file_line_budget_three_with_counted_headers_emits_ellipsis() {
    let dir = tempdir().expect("tmp");
    write_file(&dir, "a.txt", "a1\na2\na3\n");
    write_file(&dir, "b.txt", "b1\nb2\nb3\n");

    let assert = cargo_bin_cmd!("hson")
        .current_dir(dir.path())
        .args(["--no-color", "--no-sort", "-H", "-n", "6", "a.txt", "b.txt"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("==> a.txt <==\na1\na2\na3\n"),
        "counted header + cap 6 should fit full body when nothing is omitted: {stdout}"
    );
    assert!(
        stdout.contains("==> b.txt <==\nb1\nb2\nb3\n"),
        "second file should also render fully when within cap: {stdout}"
    );
    assert!(
        !stdout.contains("\n…\n"),
        "ellipsis should only appear when content is omitted: {stdout}"
    );
}
