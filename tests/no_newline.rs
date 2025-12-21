mod common;

fn run_with_flags(input: &str, template: &str, extra: &[&str]) -> String {
    common::run_template_budget_no_color(input, template, 1000, extra)
}

#[test]
fn no_newline_flag_makes_single_line() {
    // Non-trivial JSON that normally renders with newlines
    let input = r#"{"a": [1, 2, 3], "b": {"c": 1, "d": 2}}"#;
    let templates = ["json", "pseudo", "js"];

    for tmpl in templates {
        let multi = run_with_flags(input, tmpl, &[]);
        let multi_trimmed = common::trim_trailing_newlines(&multi);
        assert!(
            multi_trimmed.contains('\n'),
            "expected multi-line output for {tmpl}"
        );

        let single = run_with_flags(input, tmpl, &["--no-newline"]);
        let single_trimmed = common::trim_trailing_newlines(&single);
        assert!(
            !single_trimmed.contains('\n'),
            "expected single-line output for {tmpl}, got: {single:?}"
        );

        // Only long flag is supported for this option.

        if tmpl == "json" {
            serde_json::from_str::<serde_json::Value>(&multi)
                .expect("json (multi) should parse");
            serde_json::from_str::<serde_json::Value>(&single)
                .expect("json (single) should parse");
        }
    }
}
