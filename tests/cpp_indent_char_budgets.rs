use assert_cmd::assert::Assert;

fn run_cli_file(path: &str, args: &[&str]) -> Assert {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    cmd.args(["--no-color"]) // disable ANSI for stable snapshots
        .args(args)
        .arg(path)
        .assert()
}

#[test]
fn cpp_indent_char_budgets_snapshots() {
    let p = "tests/fixtures/code/sample.cpp";
    // Single budget for a representative summary
    let out = run_cli_file(p, &["-u", "120", "-f", "auto"]) // auto → indent-text for .cpp
        .success();
    let mut s = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    // Normalize trailing newlines for snapshot stability
    while s.ends_with('\n') { s.pop(); }
    s.push('\n');
    insta::assert_snapshot!("cpp_chars_120", s);
}
