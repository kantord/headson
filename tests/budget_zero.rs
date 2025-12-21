mod common;

#[test]
fn budget_zero_renders_single_node_minimal_output() {
    let templates = ["json", "pseudo", "js"];
    let inputs = ["[]", "{}", "\"x\"", "0", "true", "null"];
    for &tmpl in &templates {
        for &input in &inputs {
            let out =
                common::run_template_budget_no_color(input, tmpl, 0, &[]);
            let expected = "\n";
            assert_eq!(out, expected, "template={tmpl}, input={input}");
        }
    }
}
