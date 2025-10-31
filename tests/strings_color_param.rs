use insta::assert_snapshot;

fn run_color(input: &str, template: &str) -> String {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let mut args = vec!["--color", "-n", "1000"];
    let lower = template.to_ascii_lowercase();
    match lower.as_str() {
        "json" => args.extend(["-f", "json", "-t", "strict"]),
        "pseudo" => args.extend(["-f", "json", "-t", "default"]),
        "js" => args.extend(["-f", "json", "-t", "detailed"]),
        other => args.extend(["-f", other]),
    }
    let assert = cmd.args(args).write_stdin(input).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

#[test]
fn color_string_across_templates() {
    let input = "\"hello\"";
    for tmpl in ["json", "pseudo", "js"] {
        let out = run_color(input, tmpl);
        assert_snapshot!(format!("color_string_{}", tmpl), out);
    }
}

#[test]
fn color_object_key_and_value_across_templates() {
    let input = "{\"k\":\"v\"}";
    for tmpl in ["json", "pseudo", "js"] {
        let out = run_color(input, tmpl);
        assert_snapshot!(format!("color_object_kv_{}", tmpl), out);
    }
}
