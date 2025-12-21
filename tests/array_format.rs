mod common;

#[test]
fn arrays_have_no_space_after_commas_compact() {
    let input = "[1,2,3,4]";
    for &tmpl in &["json", "pseudo", "js"] {
        let out = common::run_template_budget_no_color(
            input,
            tmpl,
            1000,
            &["--compact"],
        );
        let trimmed = out.trim_end_matches(['\r', '\n']);
        assert!(trimmed.contains("[1,2,3,4]"), "compact array: {trimmed:?}");
        assert!(
            !trimmed.contains(", "),
            "no space after commas: {trimmed:?}"
        );
    }
}

#[test]
fn arrays_have_no_space_after_commas_default() {
    let input = "[1,2,3,4]";
    for &tmpl in &["json", "pseudo", "js"] {
        let out = common::run_template_budget_no_color(input, tmpl, 1000, &[]);
        assert!(
            !out.contains(", "),
            "no space after commas expected: {out:?}"
        );
    }
}
