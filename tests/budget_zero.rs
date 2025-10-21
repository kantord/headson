#[path = "../test_support/mod.rs"]
mod util;

fn expected_min_output(input: &str, template: &str) -> &'static str {
    match (input, template) {
        ("[]", _) => "[]\n",
        ("{}", _) => "{}\n",
        ("\"x\"", _) => "\"…\"\n",
        ("0", _) => "0\n",
        ("true", _) => "true\n",
        ("null", _) => "null\n",
        _ => "\n",
    }
}

#[test]
fn budget_zero_renders_single_node_minimal_output() {
    let templates = ["json", "pseudo", "js"];
    let inputs = ["[]", "{}", "\"x\"", "0", "true", "null"];
    for &tmpl in &templates {
        for &input in &inputs {
            let out = util::run_template_budget(input, tmpl, 0, &[]);
            let expected = expected_min_output(input, tmpl);
            assert_eq!(out, expected, "template={tmpl}, input={input}");
        }
    }
}
