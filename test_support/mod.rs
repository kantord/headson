use assert_cmd::{Command, assert::Assert};

#[allow(dead_code)]
pub fn run_stdout(input: &str, args: &[&str]) -> String {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd.args(args).write_stdin(input).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[allow(dead_code)]
pub fn run_template_budget(
    input: &str,
    template: &str,
    budget: usize,
    extra: &[&str],
) -> String {
    let budget_s = budget.to_string();
    let mut args = vec!["-n", &budget_s, "-f", template];
    args.extend_from_slice(extra);
    run_stdout(input, &args)
}

#[allow(dead_code)]
pub fn run_template_budget_assert(
    input: &str,
    template: &str,
    budget: usize,
    extra: &[&str],
) -> Assert {
    let budget_s = budget.to_string();
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let mut args = vec!["-n", &budget_s, "-f", template];
    args.extend_from_slice(extra);
    cmd.args(args).write_stdin(input).assert()
}
