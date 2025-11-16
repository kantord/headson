use insta::assert_snapshot;

fn run(args: &[&str]) -> String {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args(
            std::iter::once("--no-color")
                .chain(std::iter::once("--no-sort"))
                .chain(args.iter().copied()),
        )
        .assert()
        .success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

fn count_lines_normalized(s: &str) -> usize {
    if s.is_empty() {
        return 0;
    }
    // The CLI prints with println!, so stdout ends with a trailing '\n'.
    // Trim a single trailing LF to measure the internal render, then count.
    let trimmed = s.strip_suffix('\n').unwrap_or(s);
    if trimmed.is_empty() {
        0
    } else {
        trimmed.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1
    }
}

fn count_non_header_lines(s: &str) -> usize {
    s.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("==>")
        })
        .count()
}

#[test]
fn json_strict_lines_cap() {
    let p = "tests/fixtures/explicit/object_small.json";
    let out = run(&["-f", "json", "-t", "strict", "-n", "2", p]);
    assert!(
        count_lines_normalized(&out) <= 2,
        "lines cap not enforced: {out:?}"
    );
    assert_snapshot!("json_strict_lines2", out);
}

#[test]
fn json_pseudo_lines_cap() {
    let p = "tests/fixtures/explicit/object_small.json";
    let out = run(&["-f", "json", "-t", "default", "-n", "3", p]);
    assert!(
        count_lines_normalized(&out) <= 3,
        "lines cap not enforced: {out:?}"
    );
    assert_snapshot!("json_pseudo_lines3", out);
}

#[test]
fn json_js_lines_cap() {
    let p = "tests/fixtures/explicit/object_small.json";
    let out = run(&["-f", "json", "-t", "detailed", "-n", "4", p]);
    assert!(
        count_lines_normalized(&out) <= 4,
        "lines cap not enforced: {out:?}"
    );
    assert_snapshot!("json_js_lines4", out);
}

#[test]
fn yaml_lines_cap_multiline_values() {
    use std::fs;
    let tmp = tempfile::tempdir_in(".").expect("tmp");
    let p = tmp.path().join("doc.yaml");
    let doc =
        "root:\n  items: [1,2,3,4,5,6]\n  desc: \"line1\\nline2\\nline3\"\n";
    fs::write(&p, doc).unwrap();
    let path_str = p.to_string_lossy();
    let out = run(&["-i", "yaml", "-f", "yaml", "-n", "4", &path_str]);
    assert!(
        count_lines_normalized(&out) <= 4,
        "lines cap not enforced: {out:?}"
    );
    assert_snapshot!("yaml_lines4", out);
}

#[test]
fn text_lines_cap_with_omission() {
    use std::fs;
    let tmp = tempfile::tempdir_in(".").expect("tmp");
    let p = tmp.path().join("lines.txt");
    let content = (1..=10).map(|i| format!("L{i}\n")).collect::<String>();
    fs::write(&p, content).unwrap();
    let path_str = p.to_string_lossy();
    // default style shows omission line; ensure total lines <= 3
    let out = run(&["-i", "text", "-f", "text", "-n", "3", &path_str]);
    let numbered = out
        .lines()
        .filter(|line| {
            line.trim_start()
                .starts_with(|ch: char| ch.is_ascii_digit())
        })
        .count();
    assert!(numbered <= 3, "lines cap not enforced: {out:?}");
    assert_snapshot!("text_lines3_default", out);
}

#[test]
fn combined_char_and_line_caps() {
    let p = "tests/fixtures/explicit/string_escaping.json";
    // Enforce both: small byte cap and small line cap
    let out = run(&["-f", "json", "-t", "default", "-n", "2", "-c", "60", p]);
    let lines = count_lines_normalized(&out);
    assert!(lines <= 2, "line cap failed: {out:?}");
    let trimmed_len = out.trim_end_matches('\n').len();
    assert!(
        trimmed_len <= 60,
        "byte cap failed: len={trimmed_len} > 60, out={out:?}",
    );
    assert_snapshot!("json_pseudo_lines2_chars60", out);
}

#[test]
fn fileset_global_lines() {
    use std::fs;
    let tmp = tempfile::tempdir_in(".").expect("tmp");
    let a = tmp.path().join("a.json");
    let b = tmp.path().join("b.json");
    fs::write(&a, b"{}\n").unwrap();
    fs::write(&b, b"[]\n").unwrap();
    let out = run(&[
        "-f",
        "auto",
        "--global-lines",
        "3",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
    ]);
    let non_header = count_non_header_lines(&out);
    assert!(
        non_header <= 3,
        "global lines cap failed (content lines exceed cap): {out:?}"
    );
    // Should contain at least one fileset header.
    assert!(out.contains("==> "));
}

#[test]
fn lines_only_no_char_cap() {
    let p = "tests/fixtures/explicit/object_small.json";
    // No -c / -C provided; lines only should still work
    let out = run(&["-f", "json", "-t", "strict", "-n", "1", p]);
    assert!(count_lines_normalized(&out) <= 1);
}
