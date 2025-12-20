use assert_cmd::cargo::cargo_bin_cmd;

pub struct CliOut {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_cli(args: &[&str], stdin: Option<&[u8]>) -> CliOut {
    let mut cmd = cargo_bin_cmd!("hson");
    let mut cmd = cmd.args(args);
    if let Some(input) = stdin {
        cmd = cmd.write_stdin(input);
    }
    let assert = cmd.assert();
    let ok = assert.get_output().status.success();
    let stdout =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let stderr =
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    CliOut { ok, stdout, stderr }
}
