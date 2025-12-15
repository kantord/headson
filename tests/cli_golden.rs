use std::fs;
use std::path::Path;

fn stdout_from(cwd: &Path, args: &[&str], stdin: Option<&str>) -> String {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let mut cmd = cmd.current_dir(cwd).args(args);
    if let Some(input) = stdin {
        cmd = cmd.write_stdin(input);
    }
    let assert = cmd.assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

fn stdout_stderr_from(cwd: &Path, args: &[&str]) -> (String, String) {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd.current_dir(cwd).args(args).assert().success();
    (
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned(),
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned(),
    )
}

#[test]
fn cli_golden_stdin_file_and_fileset() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let dir_path = dir.path();
    fs::write(dir_path.join("single.json"), b"{\"a\":1,\"b\":2}\n").unwrap();
    fs::write(dir_path.join("a.json"), b"{\"a\":1}\n").unwrap();
    fs::write(dir_path.join("b.yaml"), b"k: 2\n").unwrap();

    let stdin_out = stdout_from(
        dir_path,
        &["--no-color", "-c", "1000", "-f", "auto", "-i", "json"],
        Some("{\"a\":1,\"b\":2}\n"),
    );
    let file_auto = stdout_from(
        dir_path,
        &["--no-color", "-c", "1000", "-f", "auto", "single.json"],
        None,
    );
    let file_json = stdout_from(
        dir_path,
        &["--no-color", "-c", "1000", "-f", "json", "single.json"],
        None,
    );
    let fileset = stdout_from(
        dir_path,
        &["--no-color", "--no-sort", "-c", "1000", "a.json", "b.yaml"],
        None,
    );

    let snap = format!(
        "STDIN (auto):\n{stdin_out}\n--\nFILE (auto):\n{file_auto}\n--\nFILE (json):\n{file_json}\n--\nFILESET (auto):\n{fileset}\n"
    );
    insta::assert_snapshot!("cli_golden_stdin_file_fileset", snap);
}

#[test]
fn cli_notices_for_grep_and_binary_skip() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let dir_path = dir.path();
    fs::write(dir_path.join("text.txt"), b"hello world\n").unwrap();
    fs::write(dir_path.join("bin.dat"), [0u8, 159, 255, 0]).unwrap();

    let (stdout, stderr) = stdout_stderr_from(
        dir_path,
        &[
            "--no-color",
            "--no-sort",
            "-c",
            "200",
            "--grep",
            "zzz",
            "text.txt",
            "bin.dat",
        ],
    );

    let snap = format!("STDOUT:\n{stdout}\nSTDERR:\n{stderr}");
    insta::assert_snapshot!("cli_grep_and_binary_notices", snap);
}
