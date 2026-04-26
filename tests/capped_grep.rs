mod common;

fn run_ok(args: &[&str], stdin: Option<&[u8]>) -> common::CliOutput {
    common::run_cli(args, stdin)
}

#[test]
fn capped_grep_flag_is_accepted_by_cli() {
    let input = br#"{"key":"needle"}"#.to_vec();
    let out = run_ok(&["--no-color", "--capped-grep", "needle"], Some(&input));
    assert!(
        !out.stdout_ansi.is_empty(),
        "--capped-grep should produce non-empty output on a matching JSON input"
    );
}

#[test]
fn capped_igrep_flag_is_accepted_by_cli() {
    let input = br#"{"key":"NEEDLE"}"#.to_vec();
    let out =
        run_ok(&["--no-color", "--capped-igrep", "NEEDLE"], Some(&input));
    assert!(
        !out.stdout_ansi.is_empty(),
        "--capped-igrep should produce non-empty output on a matching JSON input"
    );
}

#[test]
fn capped_grep_respects_budget_unlike_grep() {
    let input =
        br#"{"key": "needle value here to make output bigger"}"#.to_vec();

    let grep_out = run_ok(
        &[
            "--no-color",
            "--bytes",
            "5",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "needle",
        ],
        Some(&input),
    );
    assert!(
        grep_out.stdout.contains("needle"),
        "--grep should guarantee match inclusion; got: {:?}",
        grep_out.stdout
    );
    assert!(
        grep_out.stdout.len() > 5,
        "--grep should expand beyond the user budget; len={}",
        grep_out.stdout.len()
    );

    let capped_out = run_ok(
        &[
            "--no-color",
            "--bytes",
            "5",
            "-f",
            "json",
            "-t",
            "strict",
            "--capped-grep",
            "needle",
        ],
        Some(&input),
    );
    let within_budget = capped_out.stdout.len() <= 6;
    let no_match = !capped_out.stdout.contains("needle");
    assert!(
        within_budget || no_match,
        "--capped-grep should respect the byte budget; \
         got {} bytes with needle present: {:?}",
        capped_out.stdout.len(),
        capped_out.stdout
    );
}

#[test]
fn capped_grep_accepts_grep_show_all_without_grep_flag() {
    let input = br#"{"key": "needle"}"#;
    let out = run_ok(
        &[
            "--no-color",
            "--capped-grep",
            "needle",
            "--grep-show",
            "all",
        ],
        Some(input),
    );
    assert!(
        out.stdout.contains("needle"),
        "output should contain match when --grep-show all and --capped-grep are combined"
    );
}

#[test]
fn capped_grep_and_grep_together_is_rejected() {
    let out = common::run_cli_expect_fail(
        &["--no-color", "--grep", "needle", "--capped-grep", "other"],
        Some(b"{}"),
        None,
    );
    assert!(
        out.stderr.contains("--capped-grep") && out.stderr.contains("--grep"),
        "combining --grep with --capped-grep should produce a clear error; got: {:?}",
        out.stderr
    );
}
