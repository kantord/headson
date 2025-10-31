use assert_cmd::assert::Assert;

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_stdout(input: &str, args: &[&str]) -> String {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let assert = cmd
        .arg("--no-color")
        .args(args)
        .write_stdin(input)
        .assert()
        .success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_template_budget(
    input: &str,
    template: &str,
    budget: usize,
    extra: &[&str],
) -> String {
    let budget_s = budget.to_string();
    let mut args: Vec<&str> = vec!["-n", &budget_s];
    let lower = template.to_ascii_lowercase();
    match lower.as_str() {
        "json" => args.extend(["-f", "json", "-t", "strict"]),
        "yaml" => args.extend(["-f", "yaml"]),
        "pseudo" => args.extend(["-f", "json", "-t", "default"]),
        "js" => args.extend(["-f", "json", "-t", "detailed"]),
        other => args.extend(["-f", other]),
    }
    args.extend_from_slice(extra);
    run_stdout(input, &args)
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_template_budget_assert(
    input: &str,
    template: &str,
    budget: usize,
    extra: &[&str],
) -> Assert {
    let budget_s = budget.to_string();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let mut args: Vec<&str> = vec!["-n", &budget_s];
    let lower = template.to_ascii_lowercase();
    match lower.as_str() {
        "json" => args.extend(["-f", "json", "-t", "strict"]),
        "yaml" => args.extend(["-f", "yaml"]),
        "pseudo" => args.extend(["-f", "json", "-t", "default"]),
        "js" => args.extend(["-f", "json", "-t", "detailed"]),
        other => args.extend(["-f", other]),
    }
    args.extend_from_slice(extra);
    cmd.arg("--no-color").args(args).write_stdin(input).assert()
}

#[allow(dead_code, reason = "test helpers used ad-hoc across tests")]
pub fn run_capture(input: &[u8], args: &[&str]) -> (bool, Vec<u8>, Vec<u8>) {
    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
        .arg("--no-color")
        .args(args)
        .write_stdin(input)
        .assert();
    let ok = assert.get_output().status.success();
    let out = assert.get_output().stdout.clone();
    let err = assert.get_output().stderr.clone();
    (ok, out, err)
}
