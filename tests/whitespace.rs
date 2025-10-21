#[path = "../test_support/mod.rs"]
mod util;
use std::fs;

fn input_object() -> String {
    fs::read_to_string("tests/fixtures/explicit/object_small.json")
        .expect("read fixture")
}

#[test]
fn no_space_removes_space_after_colon_in_objects() {
    let input = input_object();
    let out = util::run_template_budget(&input, "json", 1000, &["--no-space"]);
    assert!(out.contains(":"));
    assert!(
        !out.contains(": "),
        "should not contain space after colon: {out:?}"
    );
}

#[test]
fn indent_tab_produces_tab_indentation() {
    let input = input_object();
    let out =
        util::run_template_budget(&input, "json", 1000, &["--indent", "\t"]);
    assert!(out.contains('\n'));
    assert!(out.contains("\n\t"), "expected tab indentation: {out:?}");
    assert!(
        !out.contains("\n  "),
        "should not contain two-space indentation: {out:?}"
    );
}

#[test]
fn indent_multi_char_produces_custom_indentation() {
    let input = input_object();
    let out =
        util::run_template_budget(&input, "json", 1000, &["--indent", ".."]);
    assert!(out.contains('\n'));
    assert!(out.contains("\n.."), "expected custom indent: {out:?}");
    assert!(
        !out.contains("\n  "),
        "should not contain two-space indentation: {out:?}"
    );
}
