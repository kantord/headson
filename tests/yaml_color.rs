mod common;
use headson::{ColorMode, OutputTemplate, PriorityConfig, RenderConfig};

#[test]
fn yaml_coloring_applies_and_strips_to_plain() {
    // A small YAML mapping + array exercising keys, unquoted and quoted strings, and numbers.
    let input =
        b"name: Alice\nage: 42\nlikes: [tea, \"ice cream\", 7]\n".to_vec();

    let cfg_plain = RenderConfig {
        template: OutputTemplate::Yaml,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: ColorMode::On,
        color_enabled: false,
        style: headson::Style::Default,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        fileset_tree: false,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    };
    let cfg_color = RenderConfig {
        color_enabled: true,
        ..cfg_plain.clone()
    };
    let prio = PriorityConfig::new(usize::MAX, usize::MAX);

    let budget = 10_000usize;
    let budgets = headson::Budgets {
        global: Some(headson::Budget {
            kind: headson::BudgetKind::Bytes,
            cap: budget,
        }),
        per_slot: None,
    };
    let grep = headson::GrepConfig::default();
    let plain = headson::headson(
        headson::InputKind::Yaml(input.clone()),
        &cfg_plain,
        &prio,
        &grep,
        budgets,
    )
    .expect("plain yaml")
    .text;
    let colored = headson::headson(
        headson::InputKind::Yaml(input),
        &cfg_color,
        &prio,
        &grep,
        budgets,
    )
    .expect("colored yaml")
    .text;

    // Contains ANSI SGR and specific roles (blue for keys, green for strings).
    assert!(
        colored.contains("\u{001b}["),
        "expected ANSI escapes in colored output"
    );
    assert!(
        colored.contains("\u{001b}[1;34m"),
        "expected key color (bold blue) present"
    );
    assert!(
        colored.contains("\u{001b}[32m"),
        "expected string color (green) present"
    );

    // Stripped output should be identical to the plain render.
    let stripped = common::strip_ansi(&colored);
    assert_eq!(plain, stripped);
}
