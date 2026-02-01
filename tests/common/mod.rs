use assert_cmd::cargo::cargo_bin_cmd;
use std::process::Output;

#[allow(dead_code, reason = "fields are used selectively across tests")]
pub struct CliOutput {
    pub raw: Output,
    pub stdout_ansi: String,
    pub stderr_ansi: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

impl CliOutput {
    pub fn success(&self) -> bool {
        self.raw.status.success()
    }
}

impl std::ops::Deref for CliOutput {
    type Target = Output;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

pub fn build_cmd(args: &[&str], stdin: Option<&[u8]>) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("hson");
    cmd.args(args);
    if let Some(input) = stdin {
        cmd.write_stdin(input);
    }
    cmd
}

fn capture_output(raw: Output) -> CliOutput {
    let stdout_ansi = String::from_utf8_lossy(&raw.stdout).into_owned();
    let stderr_ansi = String::from_utf8_lossy(&raw.stderr).into_owned();
    let stdout = strip_ansi(&stdout_ansi);
    let stderr = strip_ansi(&stderr_ansi);
    let exit_code = raw.status.code();
    CliOutput {
        raw,
        stdout_ansi,
        stderr_ansi,
        stdout,
        stderr,
        exit_code,
    }
}

#[allow(
    dead_code,
    reason = "test helpers are used selectively across per-test crates"
)]
/// Runs the CLI and asserts success; stdout/stderr are captured in `CliOutput`.
pub fn run_cli(args: &[&str], stdin: Option<&[u8]>) -> CliOutput {
    // Note: this helper asserts success; use *_expect_fail for non-zero exits.
    let raw = build_cmd(args, stdin).assert().get_output().clone();
    let out = capture_output(raw);
    assert!(out.success(), "cli should succeed");
    out
}

#[allow(
    dead_code,
    reason = "test helpers are used selectively across per-test crates"
)]
/// Runs the CLI in `dir` and asserts success; stdout/stderr are captured in `CliOutput`.
pub fn run_cli_in_dir(
    dir: impl AsRef<std::path::Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> CliOutput {
    // Note: this helper asserts success; use *_expect_fail for non-zero exits.
    let raw = build_cmd_in_dir(dir, args, stdin)
        .assert()
        .get_output()
        .clone();
    let out = capture_output(raw);
    assert!(out.success(), "cli should succeed");
    out
}

pub fn build_cmd_in_dir(
    dir: impl AsRef<std::path::Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("hson");
    cmd.current_dir(dir).args(args);
    if let Some(input) = stdin {
        cmd.write_stdin(input);
    }
    cmd
}

#[allow(
    dead_code,
    reason = "test helpers are used selectively across per-test crates"
)]
/// Runs the CLI in `dir` with `envs` and asserts success; stdout/stderr are captured in `CliOutput`.
pub fn run_cli_in_dir_env(
    dir: impl AsRef<std::path::Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
    envs: &[(&str, &std::ffi::OsStr)],
) -> CliOutput {
    let raw = build_cmd_in_dir_env(dir, args, stdin, envs)
        .assert()
        .get_output()
        .clone();
    let out = capture_output(raw);
    assert!(out.success(), "cli should succeed");
    out
}

pub fn build_cmd_in_dir_env(
    dir: impl AsRef<std::path::Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
    envs: &[(&str, &std::ffi::OsStr)],
) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("hson");
    cmd.current_dir(dir).args(args);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    if let Some(input) = stdin {
        cmd.write_stdin(input);
    }
    cmd
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
/// Runs the CLI with `--no-color` and asserts success; stdout/stderr are captured in `CliOutput`.
pub fn run_cli_no_color(args: &[&str], stdin: Option<&[u8]>) -> CliOutput {
    let mut with_flags: Vec<&str> = Vec::with_capacity(args.len() + 1);
    with_flags.push("--no-color");
    with_flags.extend_from_slice(args);
    run_cli(&with_flags, stdin)
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_stdout_no_color(input: &str, args: &[&str]) -> String {
    let out = run_cli_no_color(args, Some(input.as_bytes()));
    out.stdout
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_template_budget_no_color(
    input: &str,
    template: &str,
    budget: usize,
    extra: &[&str],
) -> String {
    let budget_s = budget.to_string();
    let mut args: Vec<&str> = vec!["-c", &budget_s];
    args.extend(template_args(template));
    args.extend_from_slice(extra);
    run_stdout_no_color(input, &args)
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_template_budget_assert_no_color(
    input: &str,
    template: &str,
    budget: usize,
    extra: &[&str],
) -> assert_cmd::assert::Assert {
    let budget_s = budget.to_string();
    let mut args: Vec<&str> = vec!["-c", &budget_s];
    args.extend(template_args(template));
    args.extend_from_slice(extra);
    build_cmd_no_color(&args, Some(input.as_bytes())).assert()
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_capture_no_color(
    input: &[u8],
    args: &[&str],
) -> (bool, Vec<u8>, Vec<u8>) {
    let out = run_cli_no_color(args, Some(input));
    let ok = out.success();
    (ok, out.raw.stdout, out.raw.stderr)
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_cli_expect_fail(
    args: &[&str],
    stdin: Option<&[u8]>,
    expect_code: Option<i32>,
) -> CliOutput {
    let raw = build_cmd(args, stdin).assert().get_output().clone();
    let out = capture_output(raw);
    assert!(!out.success(), "cli should fail");
    if let Some(code) = expect_code {
        assert_eq!(
            out.exit_code,
            Some(code),
            "expected exit code {code}, got {:?}",
            out.exit_code
        );
    }
    out
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_cli_in_dir_expect_fail(
    dir: impl AsRef<std::path::Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
    expect_code: Option<i32>,
) -> CliOutput {
    let raw = build_cmd_in_dir(dir, args, stdin)
        .assert()
        .get_output()
        .clone();
    let out = capture_output(raw);
    assert!(!out.success(), "cli should fail");
    if let Some(code) = expect_code {
        assert_eq!(
            out.exit_code,
            Some(code),
            "expected exit code {code}, got {:?}",
            out.exit_code
        );
    }
    out
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_cli_in_dir_env_expect_fail(
    dir: impl AsRef<std::path::Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
    envs: &[(&str, &std::ffi::OsStr)],
    expect_code: Option<i32>,
) -> CliOutput {
    let raw = build_cmd_in_dir_env(dir, args, stdin, envs)
        .assert()
        .get_output()
        .clone();
    let out = capture_output(raw);
    assert!(!out.success(), "cli should fail");
    if let Some(code) = expect_code {
        assert_eq!(
            out.exit_code,
            Some(code),
            "expected exit code {code}, got {:?}",
            out.exit_code
        );
    }
    out
}

fn build_cmd_no_color(
    args: &[&str],
    stdin: Option<&[u8]>,
) -> assert_cmd::Command {
    let mut with_flags: Vec<&str> = Vec::with_capacity(args.len() + 1);
    with_flags.push("--no-color");
    with_flags.extend_from_slice(args);
    build_cmd(&with_flags, stdin)
}

fn template_args(template: &str) -> Vec<&str> {
    let lower = template.to_ascii_lowercase();
    match lower.as_str() {
        "json" => vec!["-f", "json", "-t", "strict"],
        "yaml" => vec!["-f", "yaml"],
        "pseudo" => vec!["-f", "json", "-t", "default"],
        "js" => vec!["-f", "json", "-t", "detailed"],
        _ => vec!["-f", template],
    }
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn normalize_trailing_newline(s: &str) -> String {
    let mut out = s.to_string();
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn trim_trailing_newlines(s: &str) -> &str {
    s.trim_end_matches(['\r', '\n'])
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn strip_ansi(s: &str) -> String {
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

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn normalize_snapshot_paths(s: &str) -> String {
    s.replace('\\', "/")
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn normalize_debug(s: &str) -> String {
    use serde_json::{self, Value};
    let mut v: Value = serde_json::from_str(s).expect("stderr must be JSON");
    normalize_debug_value(&mut v);
    serde_json::to_string_pretty(&v).unwrap()
}

fn normalize_debug_value(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            normalize_debug_object(map);
            for (_k, vv) in map.iter_mut() {
                normalize_debug_value(vv);
            }
        }
        serde_json::Value::Array(arr) => {
            for vv in arr.iter_mut() {
                normalize_debug_value(vv);
            }
        }
        _ => {}
    }
}

fn normalize_debug_object(
    map: &mut serde_json::Map<String, serde_json::Value>,
) {
    if let Some(id) = map.get_mut("id") {
        *id = serde_json::Value::from(0);
    }
    if let Some(counts) = map.get_mut("counts") {
        if let Some(obj) = counts.as_object_mut() {
            obj.insert("total_nodes".to_string(), serde_json::Value::from(0));
            obj.insert("included".to_string(), serde_json::Value::from(0));
        }
    }
}
