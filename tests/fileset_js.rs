use assert_cmd::Command;
use insta::assert_snapshot;

fn run_js(paths: &[&str], budget: usize) -> String {
    let budget_s = budget.to_string();
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let mut args = vec!["-n", &budget_s, "-f", "js"]; // newline mode
    args.extend_from_slice(paths);
    let assert = cmd.args(args).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[test]
fn js_fileset_head_style_headers() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    let out = run_js(&[p1, p2, p3], 100_000);
    assert_snapshot!("js_fileset_head_style_headers", out);
}

#[test]
fn js_fileset_shows_omitted_summary_when_budget_small() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    // Use a tiny budget to ensure some files are omitted
    let out = run_js(&[p1, p2, p3], 50);
    assert!(
        out.contains("more files"),
        "expected omitted summary in output: {out:?}"
    );
}

#[test]
fn js_fileset_compact_shows_inline_omitted_summary() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let p3 = "tests/fixtures/explicit/string_escaping.json";
    let budget = 50usize;
    let budget_s = budget.to_string();
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    // Compact mode => no newlines, but object-style rendering includes inline summary
    let assert = cmd
        .args(["-n", &budget_s, "-f", "js", "--compact", p1, p2, p3])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("more files"),
        "expected inline summary: {out:?}"
    );
}

#[test]
fn js_fileset_summary_leads_with_two_blank_lines_when_no_sections() {
    // With an extremely small budget, no file sections are included (kept=0)
    // but we still render a summary. The JS fileset summary should start with
    // two blank lines to visually separate it when there was no preceding newline.
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let out = run_js(&[p1, p2], 1);
    assert!(
        out.starts_with("\n\n/*"),
        "expected two leading newlines before summary, got: {out:?}"
    );
}
