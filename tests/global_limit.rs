mod common;
#[path = "../test_support/mod.rs"]
mod util;

fn run_paths_json(paths: &[&str], args: &[&str]) -> (bool, String, String) {
    let mut full_args = vec!["--no-color", "--no-sort", "-f", "auto"];
    full_args.extend_from_slice(args);
    full_args.extend_from_slice(paths);
    let out = common::run_cli(&full_args, None);
    let ok = out.status.success();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (ok, stdout, stderr)
}

fn run_js_with_limit(paths: &[&str], limit: usize, extra: &[&str]) -> String {
    let limit_s = limit.to_string();
    let mut args = vec![
        "--no-color",
        "--no-sort",
        "-f",
        "auto",
        "-t",
        "detailed",
        "-C",
        &limit_s,
    ];
    args.extend_from_slice(extra);
    args.extend_from_slice(paths);
    let out = common::run_cli(&args, None);
    assert!(out.status.success(), "cli should succeed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn count_section_headers(out: &str) -> usize {
    out.lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with("==> "))
        .filter(|l| !l.contains(" more files "))
        .count()
}

fn find_js_summary_output(
    paths: &[&str],
    budgets: &[usize],
    extra: &[&str],
) -> Option<(String, usize)> {
    for &b in budgets {
        let out = run_js_with_limit(paths, b, extra);
        let omitted = paths.len().saturating_sub(count_section_headers(&out));
        if omitted > 0 {
            let summary = format!("==> {omitted} more files <==");
            if out.contains(&summary) {
                return Some((out, omitted));
            }
        }
    }
    None
}

fn run_pseudo_with_limit(paths: &[&str], limit: usize) -> String {
    let limit_s = limit.to_string();
    let args =
        vec!["--no-color", "-f", "auto", "-t", "default", "-C", &limit_s];
    let full_args: Vec<&str> =
        args.into_iter().chain(paths.iter().copied()).collect();
    let out = common::run_cli(&full_args, None);
    assert!(out.status.success(), "cli should succeed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn count_pseudo_headers(out: &str) -> usize {
    out.lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with("==> "))
        .filter(|l| !l.contains(" more files "))
        .count()
}

fn find_pseudo_summary_output(
    paths: &[&str],
    budgets: &[usize],
) -> Option<(String, usize)> {
    for &b in budgets {
        let out = run_pseudo_with_limit(paths, b);
        let omitted = paths.len().saturating_sub(count_pseudo_headers(&out));
        if omitted > 0 {
            let summary = format!("==> {omitted} more files <==");
            if out.contains(&summary) {
                return Some((out, omitted));
            }
        }
    }
    None
}

#[test]
fn pseudo_fileset_summary_shows_more_files_with_newlines() {
    let paths = [
        "tests/fixtures/explicit/array_numbers_50.json",
        "tests/fixtures/explicit/object_small.json",
        "tests/fixtures/explicit/string_escaping.json",
    ];
    let budgets = [20usize, 40, 60, 80, 100, 120];
    let Some((out, omitted)) = find_pseudo_summary_output(&paths, &budgets)
    else {
        panic!("expected some budget to omit files and show pseudo summary");
    };
    let summary = format!("==> {omitted} more files <==");
    // CLI prints a trailing newline; ensure the content ends with summary
    let trimmed = out.trim_end_matches('\n');
    assert!(
        trimmed.ends_with(&summary),
        "summary must be final content line"
    );
    // Ensure there is exactly one blank line before the summary
    if let Some(pos) = trimmed.rfind(&summary) {
        let before = &trimmed[..pos];
        assert!(
            before.ends_with("\n\n"),
            "expected exactly one blank line before summary"
        );
    } else {
        panic!("summary not found in output");
    }
}

#[test]
fn global_limit_can_omit_entire_files() {
    let paths = [
        "tests/fixtures/explicit/array_numbers_50.json",
        "tests/fixtures/explicit/object_small.json",
        "tests/fixtures/explicit/string_escaping.json",
    ];
    // Impose a small global limit so not all files fit.
    let (ok, out, err) = run_paths_json(&paths, &["-C", "80"]);
    assert!(ok, "should succeed: {err}");
    let kept = count_section_headers(&out);
    assert!(kept < paths.len(), "expected some files omitted: {out}");
}

#[test]
fn budget_and_global_limit_can_be_used_together() {
    let path = "tests/fixtures/explicit/object_small.json";
    // When both are set, the effective global limit is min(c, C).
    // Here min(200, 100) = 100; using both should match using only -C 100.
    let out_both = common::run_cli(
        &["--no-color", "-f", "json", "-c", "200", "-C", "100", path],
        None,
    );
    assert!(out_both.status.success(), "cli should succeed");
    let stdout_both = String::from_utf8_lossy(&out_both.stdout).into_owned();

    let out_global_only = common::run_cli(
        &["--no-color", "-f", "json", "-C", "100", path],
        None,
    );
    assert!(out_global_only.status.success(), "cli should succeed");
    let stdout_global_only =
        String::from_utf8_lossy(&out_global_only.stdout).into_owned();

    assert_eq!(
        stdout_both, stdout_global_only,
        "combined limits should behave like -C=min(c,C)"
    );
}

#[test]
fn js_fileset_summary_shows_more_files_with_newlines() {
    let paths = [
        "tests/fixtures/explicit/array_numbers_50.json",
        "tests/fixtures/explicit/object_small.json",
        "tests/fixtures/explicit/string_escaping.json",
    ];
    let budgets = [20usize, 40, 60, 80, 100, 120];
    let (out, omitted) = find_js_summary_output(&paths, &budgets, &[])
        .expect("expected some budget to omit files and show summary");
    let summary = format!("==> {omitted} more files <==");
    // Ensure exactly one blank line before the summary
    let trimmed = out.trim_end_matches('\n');
    if let Some(pos) = trimmed.rfind(&summary) {
        let before = &trimmed[..pos];
        assert!(
            before.ends_with("\n\n"),
            "expected exactly one blank line before summary"
        );
        assert!(
            !before.ends_with("\n\n\n"),
            "should not have more than one blank line before summary"
        );
    }
}

#[test]
fn js_fileset_omission_uses_files_label_with_no_newline() {
    // Force object-style fileset rendering by disabling newlines.
    let paths = [
        "tests/fixtures/explicit/array_numbers_50.json",
        "tests/fixtures/explicit/object_small.json",
        "tests/fixtures/explicit/string_escaping.json",
    ];
    let budgets = [40usize, 60, 80, 100, 120];
    let mut found = false;
    for b in budgets {
        let out = run_js_with_limit(&paths, b, &["--no-newline"]);
        if out.contains("more files") {
            assert!(
                !out.contains("more properties"),
                "should not use 'properties' label for fileset root"
            );
            found = true;
            break;
        }
    }
    assert!(found, "expected 'more files' label under some small budget");
}
