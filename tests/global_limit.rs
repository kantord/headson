use assert_cmd::Command;
use serde_json::Value;

#[path = "../test_support/mod.rs"]
mod util;

fn run_paths_json(paths: &[&str], args: &[&str]) -> (bool, String, String) {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let mut full_args = vec!["-f", "json"];
    full_args.extend_from_slice(args);
    full_args.extend_from_slice(paths);
    let assert = cmd.args(full_args).assert();
    let ok = assert.get_output().status.success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let err =
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    (ok, out, err)
}

fn run_js_with_limit(paths: &[&str], limit: usize, extra: &[&str]) -> String {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let limit_s = limit.to_string();
    let mut args = vec!["-f", "js", "-N", &limit_s];
    args.extend_from_slice(extra);
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert();
    assert!(assert.get_output().status.success());
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

fn count_js_headers(out: &str) -> usize {
    out.lines()
        .filter(|l| l.trim_start().starts_with("// "))
        .count()
}

fn run_pseudo_with_limit(paths: &[&str], limit: usize) -> String {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let limit_s = limit.to_string();
    let args = vec!["-f", "pseudo", "-N", &limit_s];
    let assert = cmd
        .args(args.into_iter().chain(paths.iter().copied()))
        .assert();
    assert!(assert.get_output().status.success());
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
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
    let (ok, out, err) = run_paths_json(&paths, &["-N", "120"]);
    assert!(ok, "should succeed: {err}");
    let v: Value = serde_json::from_str(&out).expect("json parse");
    let obj = v.as_object().expect("root object");
    assert!(
        obj.len() < paths.len(),
        "expected some files omitted: {out}"
    );
}

#[test]
fn budget_and_global_limit_conflict() {
    let paths = ["tests/fixtures/explicit/object_small.json"];
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .args(["-f", "json", "-n", "200", "-N", "100", paths[0]])
        .assert();
    assert!(!assert.get_output().status.success(), "should fail");
}

#[test]
fn js_fileset_summary_shows_more_files_with_newlines() {
    let paths = [
        "tests/fixtures/explicit/array_numbers_50.json",
        "tests/fixtures/explicit/object_small.json",
        "tests/fixtures/explicit/string_escaping.json",
    ];
    let budgets = [20usize, 40, 60, 80, 100, 120];
    let mut found = false;
    for b in budgets {
        let out = run_js_with_limit(&paths, b, &[]);
        let omitted = paths.len().saturating_sub(count_js_headers(&out));
        if omitted > 0 {
            let summary = format!("/* {omitted} more files */");
            assert!(
                out.contains(&summary),
                "expected fileset summary: {summary}\nactual:\n{out}"
            );
            found = true;
            break;
        }
    }
    assert!(found, "expected some budget to omit files and show summary");
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
