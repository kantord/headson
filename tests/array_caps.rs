use assert_cmd::Command;

fn run_array_case(
    len: usize,
    template: &str,
    budget: usize,
    extra: &[&str],
) -> String {
    // Build a compact JSON array: [0,1,2,...]
    let mut s = String::from("[");
    for i in 0..len {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&i.to_string());
    }
    s.push(']');

    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let budget_s = budget.to_string();
    let mut args = vec!["-n", &budget_s, "-f", template, "--compact"];
    args.extend_from_slice(extra);
    let assert = cmd.args(args).write_stdin(s).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

fn parse_js_kept_omitted(out_js: &str) -> (usize, usize) {
    assert!(out_js.starts_with('[') && out_js.ends_with("]\n"));
    let body = &out_js[1..out_js.len() - 2];
    let (left, comment) = body.split_once("/*").expect("has comment");
    let kept = if left.trim().is_empty() {
        0
    } else {
        left.bytes().filter(|&b| b == b',').count() + 1
    };
    let digits: String =
        comment.chars().filter(|c| c.is_ascii_digit()).collect();
    let omitted = digits.parse::<usize>().expect("parse omitted");
    (kept, omitted)
}

#[test]
fn array_truncated_js_kept_plus_omitted_equals_total() {
    let len = 20usize;
    let budget = 30usize; // parse cap = 15
    let out_js = run_array_case(len, "js", budget, &[]);
    let (kept, omitted) = parse_js_kept_omitted(&out_js);
    assert_eq!(kept + omitted, len, "kept+omitted must equal total");
}

#[test]
fn array_truncated_pseudo_has_ellipsis() {
    let len = 20usize;
    let budget = 30usize;
    let out_pseudo = run_array_case(len, "pseudo", budget, &[]);
    assert!(out_pseudo.starts_with('[') && out_pseudo.ends_with("]\n"));
    assert!(
        out_pseudo.contains('…'),
        "expected ellipsis: {out_pseudo:?}"
    );
}

#[test]
fn array_truncated_json_length_within_cap() {
    let len = 20usize;
    let budget = 30usize;
    let out_json = run_array_case(len, "json", budget, &[]);
    let v: serde_json::Value =
        serde_json::from_str(&out_json).expect("json parse");
    let arr = v.as_array().expect("root array");
    assert!(
        arr.len() <= budget / 2,
        "array length should be <= parse cap"
    );
}
