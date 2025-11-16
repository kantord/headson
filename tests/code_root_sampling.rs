use assert_cmd::cargo::cargo_bin_cmd;
use insta::assert_snapshot;
use std::collections::HashMap;
use std::fs;
use unicode_segmentation::UnicodeSegmentation;

/// Shared code fixture set for multi-file fairness tests.
const CODE_FILESET_PATHS: &[&str] = &[
    "tests/fixtures/code/big_sample.py",
    "tests/fixtures/code/sample.py",
    "tests/fixtures/code/sample.ts",
];

fn run_sample_py_auto() -> String {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-c",
            "120",
            "-f",
            "auto",
            "tests/fixtures/code/sample.py",
        ])
        .assert()
        .success();
    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

fn run_sample_py_colored() -> String {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--color",
            "-c",
            "120",
            "-f",
            "auto",
            "tests/fixtures/code/sample.py",
        ])
        .assert()
        .success();
    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

fn run_large_code_huge_budget() -> String {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-c",
            "1000000",
            "-n",
            "1000000",
            "-f",
            "auto",
            "tests/fixtures/code/big_sample.py",
        ])
        .assert()
        .success();
    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

fn run_minimal_drop_huge_budget() -> String {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-n",
            "1000000",
            "-f",
            "auto",
            "tests/fixtures/code/minimal_drop_case.py",
        ])
        .assert()
        .success();
    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

fn run_multi_describe_line_budget() -> String {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-n",
            "1000000",
            "-f",
            "auto",
            "tests/fixtures/code/multi_describe.test.js",
        ])
        .assert()
        .success();
    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

fn run_multi_code_files_colored() -> String {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--color",
            "-c",
            "200",
            "-f",
            "auto",
            "tests/fixtures/code/sample.py",
            "tests/fixtures/code/sample.ts",
        ])
        .assert()
        .success();
    String::from_utf8_lossy(&assert.get_output().stdout).to_string()
}

fn run_code_fileset_with_budget(budget: usize) -> String {
    let mut cmd = cargo_bin_cmd!("hson");
    cmd.arg("--no-color")
        .arg("--no-sort")
        .arg("-c")
        .arg(budget.to_string())
        .arg("-f")
        .arg("auto");
    for path in CODE_FILESET_PATHS {
        cmd.arg(path);
    }
    let assert = cmd.assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).to_string()
}

fn run_large_code_fileset_small_budget() -> String {
    run_code_fileset_with_budget(120)
}

fn parse_section_header(line: &str) -> Option<&str> {
    (line.starts_with("==>") && line.ends_with("<==")).then(|| {
        line.trim()
            .trim_start_matches("==>")
            .trim_end_matches("<==")
            .trim()
    })
}

fn is_numbered_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
        && trimmed.contains(':')
}

fn push_finished_section(
    current: &mut Option<(String, usize)>,
    counts: &mut Vec<(String, usize)>,
) {
    if let Some(section) = current.take() {
        counts.push(section);
    }
}

fn handle_section_line(
    line: &str,
    current: &mut Option<(String, usize)>,
    counts: &mut Vec<(String, usize)>,
) {
    if let Some(name) = parse_section_header(line) {
        push_finished_section(current, counts);
        *current = Some((name.to_string(), 0));
    } else if let Some((_, count)) = current.as_mut() {
        if is_numbered_line(line) {
            *count += 1;
        }
    }
}

fn fileset_section_line_counts(output: &str) -> Vec<(String, usize)> {
    let mut counts = Vec::new();
    let mut current: Option<(String, usize)> = None;

    for line in output.lines() {
        handle_section_line(line, &mut current, &mut counts);
    }
    push_finished_section(&mut current, &mut counts);
    counts
}

fn fileset_section_lines<'a>(output: &'a str, section: &str) -> Vec<&'a str> {
    let mut lines = Vec::new();
    let mut in_section = false;
    for line in output.lines() {
        if let Some(name) = parse_section_header(line) {
            if in_section {
                break;
            }
            in_section = name == section;
            continue;
        }
        if in_section && !line.trim().is_empty() {
            lines.push(line);
        }
    }
    lines
}

fn count_numbered_lines(output: &str) -> usize {
    output
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("==>")
                && trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
                && trimmed.contains(':')
        })
        .count()
}

fn sanitize_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch == '\u{001b}' {
            out.push_str("\\u{001b}");
        } else {
            out.push(ch);
        }
    }
    out
}

fn strip_ansi(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            while i < bytes.len() {
                let b = bytes[i];
                i += 1;
                if b == b'm' {
                    break;
                }
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("valid utf8 after strip")
}

fn line_number_prefix(line: &str, skip_headers: bool) -> Option<&str> {
    if skip_headers {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("==>") {
            return None;
        }
    }
    let (prefix, _) = line.split_once(':')?;
    if prefix.trim().is_empty() {
        return None;
    }
    Some(prefix)
}

fn assert_line_numbers_plain(output: &str, skip_headers: bool) {
    for line in output.lines() {
        if let Some(prefix) = line_number_prefix(line, skip_headers) {
            assert!(
                !prefix.contains("\u{001b}["),
                "line number prefix contains ANSI color: {line:?}"
            );
        }
    }
}

#[test]
fn code_auto_sample_snapshot() {
    let out = run_sample_py_auto();
    assert_snapshot!("code_auto_sample_snapshot", out);
}

#[test]
fn code_auto_sample_color_snapshot() {
    let out = run_sample_py_colored();
    assert_snapshot!(
        "code_auto_sample_colorized_snapshot",
        sanitize_escapes(&out)
    );
}

#[test]
fn code_auto_sample_stripped_matches_plain_snapshot() {
    let colored = run_sample_py_colored();
    let stripped = strip_ansi(&colored);
    let plain = run_sample_py_auto();
    assert_eq!(plain, stripped, "strip_ansi should match --no-color output");
    assert_snapshot!("code_auto_sample_colorized_stripped_snapshot", stripped);
}

#[test]
fn code_line_numbers_remain_plain() {
    let colored = run_sample_py_colored();
    assert_line_numbers_plain(&colored, false);
}

#[test]
fn fileset_code_line_numbers_remain_plain() {
    let colored = run_multi_code_files_colored();
    assert_line_numbers_plain(&colored, true);
}

#[test]
fn fileset_code_small_budget_keeps_each_file_nonempty() {
    let out = run_large_code_fileset_small_budget();
    let counts = fileset_section_line_counts(&out);
    let missing: Vec<String> = counts
        .into_iter()
        .filter_map(|(name, count)| if count == 0 { Some(name) } else { None })
        .collect();
    assert!(
        missing.is_empty(),
        "expected every fileset section to include at least one line, missing: {missing:?}\n{out}"
    );
}

#[test]
fn code_fileset_respects_total_byte_budget() {
    let per_file_budget = 10;
    let out = run_code_fileset_with_budget(per_file_budget);
    let total_cap = per_file_budget * CODE_FILESET_PATHS.len();
    let trimmed_len = out.trim_end_matches('\n').len();
    assert!(
        trimmed_len <= total_cap,
        "expected total output <= {total_cap} bytes, got {trimmed_len}\n{out}"
    );
}

#[test]
fn code_multi_describe_reports_all_cases() {
    let out = run_multi_describe_line_budget();
    assert!(
        out.contains("case 5"),
        "expected later test cases to be present, got:\n{out}"
    );
}

#[test]
fn code_huge_budget_snapshot() {
    let out = run_large_code_huge_budget();
    assert_snapshot!("code_huge_budget_snapshot", out);
}

#[test]
fn code_minimal_huge_budget_snapshot() {
    let out = run_minimal_drop_huge_budget();
    assert_snapshot!("code_minimal_huge_budget_snapshot", out);
}

#[test]
fn code_prefers_top_level_headers() {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-c",
            "120",
            "-f",
            "auto",
            "tests/fixtures/code/sample.py",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("def main"),
        "expected top-level def main to appear:\n{out}"
    );
    assert!(
        out.contains("def compute"),
        "expected top-level def compute to appear:\n{out}"
    );
}

#[test]
fn fileset_line_budget_should_not_drop_code_lines_for_headers() {
    let files = [
        "tests/fixtures/code/line_budget_alpha.py",
        "tests/fixtures/code/line_budget_beta.py",
    ];
    let expected: HashMap<String, usize> = files
        .iter()
        .map(|path| {
            let contents =
                fs::read_to_string(path).expect("fixture should exist");
            (path.to_string(), contents.lines().count())
        })
        .collect();
    let per_file_budget = expected
        .values()
        .copied()
        .max()
        .expect("fixtures should contain at least one line");
    let mut cmd = cargo_bin_cmd!("hson");
    cmd.arg("--no-color")
        .arg("--no-sort")
        .arg("-n")
        .arg(per_file_budget.to_string());
    for path in &files {
        cmd.arg(path);
    }
    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let observed: HashMap<String, usize> =
        fileset_section_line_counts(&out).into_iter().collect();
    for (path, expected_lines) in expected {
        let Some(actual) = observed.get(&path) else {
            panic!("missing fileset section for {path}\n{out}");
        };
        assert_eq!(
            *actual, expected_lines,
            "expected {path} to render {expected_lines} numbered lines with -n{per_file_budget}, got {actual}\n{out}"
        );
    }
}

#[test]
fn fileset_line_budget_keeps_go_functions_in_filesets() {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-sort",
            "-n",
            "3",
            "tests/fixtures/code/sample.go",
            "tests/fixtures/code/sample.py",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let go_lines =
        fileset_section_lines(&out, "tests/fixtures/code/sample.go");
    assert!(
        go_lines
            .iter()
            .any(|line| line.contains("func compute(n int) int")),
        "expected sample.go section to retain func compute header\n{out}"
    );
    assert!(
        go_lines.iter().any(|line| line.contains("func main()")),
        "expected sample.go section to retain func main header\n{out}"
    );
}

#[test]
fn fileset_no_header_flag_hides_section_headers() {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--no-header",
            "--no-sort",
            "-n",
            "20",
            "tests/fixtures/code/sample.py",
            "tests/fixtures/code/sample.ts",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(
        !out.contains("==>"),
        "expected --no-header output to omit fileset headers:\n{out}"
    );
    assert!(
        out.contains("def greet(name: str):"),
        "python content missing from --no-header output:\n{out}"
    );
    assert!(
        out.contains("function greet(name: string) {"),
        "ts content missing from --no-header output:\n{out}"
    );
}

#[test]
fn fileset_line_budget_global_line_count_matches_expectation() {
    let files = [
        "tests/fixtures/code/sample.py",
        "tests/fixtures/code/sample.ts",
        "tests/fixtures/code/sample.go",
        "tests/fixtures/code/sample.cpp",
    ];
    let per_file_lines = 3;
    let mut cmd = cargo_bin_cmd!("hson");
    cmd.arg("--no-color")
        .arg("--no-sort")
        .arg("-n")
        .arg(per_file_lines.to_string());
    for path in &files {
        cmd.arg(path);
    }
    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let numbered = count_numbered_lines(&out);
    assert_eq!(
        numbered,
        per_file_lines * files.len(),
        "expected total numbered lines to match files * per-file cap\n{out}"
    );
}

#[test]
fn code_lines_are_hard_truncated_end_to_end() {
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-f",
            "auto",
            "tests/fixtures/code/long_line.rs",
        ])
        .assert()
        .success();
    let stdout =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let line = stdout
        .lines()
        .find(|l| l.contains("let message"))
        .expect("line present");
    let trimmed = line
        .split_once(':')
        .map(|(_, rest)| rest.trim_start())
        .unwrap_or(line);
    assert!(
        trimmed.ends_with('…'),
        "expected ellipsis suffix for long code line, got: {trimmed}"
    );
    let graphemes = UnicodeSegmentation::graphemes(trimmed, true).count();
    assert!(
        graphemes <= 151,
        "expected hard cap at ~150 graphemes (+ ellipsis); got {graphemes}"
    );
}
