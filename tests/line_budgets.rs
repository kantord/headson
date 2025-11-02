use insta::assert_snapshot;

fn run(args: &[&str]) -> String {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd.arg("--no-color").args(args).assert().success();
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

#[test]
fn json_strict_lines_cap() {
    let p = "tests/fixtures/explicit/object_small.json";
    let out = run(&["-f", "json", "-t", "strict", "--lines", "2", p]);
    assert!(
        count_lines_normalized(&out) <= 2,
        "lines cap not enforced: {out:?}"
    );
    assert_snapshot!("json_strict_lines2", out);
}

#[test]
fn json_pseudo_lines_cap() {
    let p = "tests/fixtures/explicit/object_small.json";
    let out = run(&["-f", "json", "-t", "default", "--lines", "3", p]);
    assert!(
        count_lines_normalized(&out) <= 3,
        "lines cap not enforced: {out:?}"
    );
    assert_snapshot!("json_pseudo_lines3", out);
}

#[test]
fn json_js_lines_cap() {
    let p = "tests/fixtures/explicit/object_small.json";
    let out = run(&["-f", "json", "-t", "detailed", "--lines", "4", p]);
    assert!(
        count_lines_normalized(&out) <= 4,
        "lines cap not enforced: {out:?}"
    );
    assert_snapshot!("json_js_lines4", out);
}

#[test]
fn json_strict_crlf_lines_cap_via_lib() {
    use headson::{
        Budgets, OutputTemplate, PriorityConfig, RenderConfig, Style,
    };
    let json = b"{\"a\":[1,2,3],\"b\":{\"x\":1}}".to_vec();
    let cfg = RenderConfig {
        template: OutputTemplate::Json,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\r\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: headson::ColorMode::Off,
        color_enabled: false,
        style: Style::Strict,
    };
    let prio = PriorityConfig::new(usize::MAX, usize::MAX);
    let out = headson::headson_with_budgets(
        json,
        &cfg,
        &prio,
        Budgets {
            char_budget: None,
            line_budget: Some(3),
        },
    )
    .unwrap();
    // Output should contain CRLF and have <= 3 lines (normalized).
    assert!(out.contains("\r\n"));
    let mut normalized = out.replace("\r\n", "\n");
    normalized.push('\n');
    let lines = count_lines_normalized(&normalized);
    assert!(lines <= 3);
}

#[test]
fn json_strict_no_newline_mode_lines_semantics() {
    let p = "tests/fixtures/explicit/object_small.json";
    let out = run(&[
        "-f",
        "json",
        "-t",
        "strict",
        "--no-newline",
        "--lines",
        "1",
        p,
    ]);
    // No-newline mode: treat non-empty output as 1 line.
    assert_eq!(count_lines_normalized(&out), 1);
}

#[test]
fn mixed_fileset_auto_global_lines() {
    use std::fs;
    let tmp = tempfile::tempdir_in(".").expect("tmp");
    let a = tmp.path().join("a.json");
    let b = tmp.path().join("b.yaml");
    let c = tmp.path().join("c.txt");
    fs::write(&a, b"{\"k\":1}\n").unwrap();
    fs::write(&b, b"root: 1\n").unwrap();
    fs::write(&c, b"L1\nL2\n").unwrap();
    let out = run(&[
        "-f",
        "auto",
        "--global-lines",
        "4",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        c.to_str().unwrap(),
    ]);
    assert!(count_lines_normalized(&out) <= 4);
    assert!(out.contains("==> "));
}

#[test]
fn text_tail_places_omission_at_start_under_lines_cap() {
    use std::fs;
    let tmp = tempfile::tempdir_in(".").expect("tmp");
    let p = tmp.path().join("lines.txt");
    let content = (1..=10).map(|i| format!("L{i}\n")).collect::<String>();
    fs::write(&p, content).unwrap();
    let out = run(&[
        "-i",
        "text",
        "-f",
        "text",
        "--lines",
        "3",
        "--tail",
        p.to_str().unwrap(),
    ]);
    let trimmed = out.trim_end_matches('\n');
    let mut iter = trimmed.lines();
    let first = iter.next().unwrap_or("");
    assert_eq!(first, "…", "expected omission marker at start in tail mode");
}

#[test]
fn compact_mode_long_single_line_guard_with_lines_cap() {
    // Long compact JSON with high char cap and small line cap should still be 1 line.
    let json = format!(
        "{{{}}}",
        (0..200)
            .map(|i| format!("\"k{i}\":{i}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let tmp = tempfile::tempdir_in(".").expect("tmp");
    let p = tmp.path().join("long.json");
    std::fs::write(&p, json).unwrap();
    let out = run(&[
        "-f",
        "json",
        "-t",
        "strict",
        "--compact",
        "--lines",
        "1",
        "-n",
        "100000",
        p.to_str().unwrap(),
    ]);
    assert_eq!(count_lines_normalized(&out), 1);
    assert!(out.trim_end_matches('\n').len() > 100);
}

#[test]
fn text_styles_under_line_caps() {
    use std::fs;
    let tmp = tempfile::tempdir_in(".").expect("tmp");
    let p = tmp.path().join("lines.txt");
    let content = (1..=10).map(|i| format!("L{i}\n")).collect::<String>();
    fs::write(&p, content).unwrap();
    // Strict style: should not emit the detailed omission phrase; string-level
    // ellipsis may still appear due to grapheme truncation.
    let out_strict = run(&[
        "-i",
        "text",
        "-f",
        "text",
        "-t",
        "strict",
        "--lines",
        "3",
        p.to_str().unwrap(),
    ]);
    assert!(!out_strict.contains("more lines"));
    // Detailed style: omission shows "more lines"
    let out_det = run(&[
        "-i",
        "text",
        "-f",
        "text",
        "-t",
        "detailed",
        "--lines",
        "3",
        p.to_str().unwrap(),
    ]);
    assert!(out_det.contains("more lines"));
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
    let out = run(&["-i", "yaml", "-f", "yaml", "--lines", "4", &path_str]);
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
    let out = run(&["-i", "text", "-f", "text", "--lines", "3", &path_str]);
    assert!(
        count_lines_normalized(&out) <= 3,
        "lines cap not enforced: {out:?}"
    );
    assert_snapshot!("text_lines3_default", out);
}

#[test]
fn combined_char_and_line_caps() {
    let p = "tests/fixtures/explicit/string_escaping.json";
    // Enforce both: small char cap and small line cap
    let out =
        run(&["-f", "json", "-t", "default", "--lines", "2", "-n", "60", p]);
    let lines = count_lines_normalized(&out);
    assert!(lines <= 2, "line cap failed: {out:?}");
    let trimmed_len = out.trim_end_matches('\n').len();
    assert!(
        trimmed_len <= 60,
        "char cap failed: len={trimmed_len} > 60, out={out:?}",
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
        "json",
        "--global-lines",
        "3",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
    ]);
    let lines = count_lines_normalized(&out);
    assert!(lines <= 3, "global lines cap failed: {out:?}");
    // Should contain at least one fileset header.
    assert!(out.contains("==> "));
}

#[test]
fn lines_only_no_char_cap() {
    let p = "tests/fixtures/explicit/object_small.json";
    // No -n / -N provided; lines only should still work
    let out = run(&["-f", "json", "-t", "strict", "--lines", "1", p]);
    assert!(count_lines_normalized(&out) <= 1);
}
