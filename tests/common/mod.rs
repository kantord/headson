use assert_cmd::cargo::cargo_bin_cmd;
use std::process::Output;

pub fn build_cmd(args: &[&str], stdin: Option<&[u8]>) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("hson");
    cmd.args(args);
    if let Some(input) = stdin {
        cmd.write_stdin(input);
    }
    cmd
}

#[allow(
    dead_code,
    reason = "test helpers are used selectively across per-test crates"
)]
pub fn run_cli(args: &[&str], stdin: Option<&[u8]>) -> Output {
    build_cmd(args, stdin).assert().get_output().clone()
}

#[allow(
    dead_code,
    reason = "test helpers are used selectively across per-test crates"
)]
pub fn run_cli_in_dir(
    dir: impl AsRef<std::path::Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Output {
    build_cmd_in_dir(dir, args, stdin)
        .assert()
        .get_output()
        .clone()
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
pub fn run_cli_in_dir_env(
    dir: impl AsRef<std::path::Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
    envs: &[(&str, &std::ffi::OsStr)],
) -> Output {
    build_cmd_in_dir_env(dir, args, stdin, envs)
        .assert()
        .get_output()
        .clone()
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
pub fn run_cli_no_color(args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut with_flags: Vec<&str> = Vec::with_capacity(args.len() + 1);
    with_flags.push("--no-color");
    with_flags.extend_from_slice(args);
    run_cli(&with_flags, stdin)
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_stdout_no_color(input: &str, args: &[&str]) -> String {
    let out = run_cli_no_color(args, Some(input.as_bytes()));
    assert!(out.status.success(), "cli should succeed");
    String::from_utf8_lossy(&out.stdout).into_owned()
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
    let ok = out.status.success();
    (ok, out.stdout, out.stderr)
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
