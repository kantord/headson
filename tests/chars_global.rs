use assert_cmd::assert::Assert;

fn run_cli_stdin(input: &str, args: &[&str]) -> Assert {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    cmd.args(["--no-color"])
        .args(args)
        .write_stdin(input)
        .assert()
}

fn count_chars_normalized(s: &str) -> usize {
    s.trim_end_matches('\n').chars().count()
}

#[test]
fn single_input_chars_and_global_chars_min_applies() {
    // Single input; effective cap is min(per-file, global)
    let input = "{\"a\": [1,2,3,4,5,6,7,8,9,10], \"b\": [\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\"]}";
    // Use stdin and strict JSON for determinism
    let assert = run_cli_stdin(
        input,
        &["-u", "120", "-U", "60", "-f", "json", "-t", "strict"],
    )
    .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        count_chars_normalized(&out) <= 60,
        "expected output chars <= 60, got {}\n{}",
        count_chars_normalized(&out),
        out
    );
}

#[test]
fn multi_file_chars_and_global_chars_min_applies() {
    // Two inputs; effective cap is min(per-file * N, global)
    use std::fs;
    let tmp = tempfile::tempdir().expect("tmp");
    let a = tmp.path().join("a.json");
    let b = tmp.path().join("b.json");
    fs::write(&a, b"[1,2,3,4,5,6,7,8,9,10]").unwrap();
    fs::write(&b, b"[1,2,3,4,5,6,7,8,9,10]").unwrap();

    // per-file 80, two files => 160; global 100 => expect <=100
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .args([
            "--no-color",
            "-u",
            "80",
            "-U",
            "100",
            "-f",
            "json",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        count_chars_normalized(&out) <= 100,
        "expected output chars <= 100, got {}\n{}",
        count_chars_normalized(&out),
        out
    );
}
