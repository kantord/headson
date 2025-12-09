#[path = "../test_support/mod.rs"]
mod util;

#[test]
fn budget_zero_renders_single_node_minimal_output() {
    let templates = ["json", "pseudo", "js"];
    let inputs = ["[]", "{}", "\"x\"", "0", "true", "null"];
    for &tmpl in &templates {
        for &input in &inputs {
            let out = util::run_template_budget(input, tmpl, 0, &[]);
            let expected = "\n";
            assert_eq!(out, expected, "template={tmpl}, input={input}");
        }
    }
}
