use assert_cmd::cargo::cargo_bin_cmd;
use std::process::Output;

pub fn run_cli(args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut cmd = cargo_bin_cmd!("hson");
    let mut cmd = cmd.args(args);
    if let Some(input) = stdin {
        cmd = cmd.write_stdin(input);
    }
    cmd.assert().get_output().clone()
}
