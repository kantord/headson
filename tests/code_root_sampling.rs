use assert_cmd::cargo::cargo_bin_cmd;
use insta::assert_snapshot;

fn run_sample_py_auto() -> String {
    let assert = cargo_bin_cmd!("headson")
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
    let assert = cargo_bin_cmd!("headson")
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
    for line in colored.lines() {
        if let Some(colon_idx) = line.find(':') {
            let prefix = &line[..colon_idx];
            assert!(
                !prefix.contains("\u{001b}["),
                "line number prefix contains ANSI color: {line:?}"
            );
        }
    }
}

#[test]
fn code_prefers_top_level_headers() {
    let assert = cargo_bin_cmd!("headson")
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
