use assert_cmd::cargo::cargo_bin_cmd;
use std::process::Output;

#[allow(
    dead_code,
    reason = "test helpers are used selectively across per-test crates"
)]
pub fn run_cli(args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut cmd = cargo_bin_cmd!("hson");
    let mut cmd = cmd.args(args);
    if let Some(input) = stdin {
        cmd = cmd.write_stdin(input);
    }
    cmd.assert().get_output().clone()
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
    let mut cmd = cargo_bin_cmd!("hson");
    let mut cmd = cmd.current_dir(dir).args(args);
    if let Some(input) = stdin {
        cmd = cmd.write_stdin(input);
    }
    cmd.assert().get_output().clone()
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
    let mut cmd = cargo_bin_cmd!("hson");
    let mut cmd = cmd.current_dir(dir).args(args);
    for (key, value) in envs {
        cmd = cmd.env(key, value);
    }
    if let Some(input) = stdin {
        cmd = cmd.write_stdin(input);
    }
    cmd.assert().get_output().clone()
}
