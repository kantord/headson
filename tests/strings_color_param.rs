mod common;
use insta::assert_snapshot;

fn run_color(input: &str, template: &str) -> String {
    let mut args = vec!["--color", "-c", "1000"];
    let lower = template.to_ascii_lowercase();
    match lower.as_str() {
        "json" => args.extend(["-f", "json", "-t", "strict"]),
        "pseudo" => args.extend(["-f", "json", "-t", "default"]),
        "js" => args.extend(["-f", "json", "-t", "detailed"]),
        other => args.extend(["-f", other]),
    }
    let out = common::run_cli(&args, Some(input.as_bytes()));
    assert!(
        out.status.success(),
        "cli should succeed for template {template}"
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
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
