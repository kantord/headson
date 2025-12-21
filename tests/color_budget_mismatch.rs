mod common;

#[test]
fn colored_and_plain_outputs_should_match_after_stripping() {
    // Arrange a small array whose render sits near the byte budget edge.
    // Coloring adds ANSI SGR sequences to strings, which do not count toward
    // the budget: measuring is done on uncolored output, so inclusion is
    // identical after stripping colors.
    let input =
        b"[\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\"]";

    let cfg_plain = headson::RenderConfig {
        template: headson::OutputTemplate::Json,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: headson::ColorMode::On,
        color_enabled: false,
        style: headson::Style::Strict,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        fileset_tree: false,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    };
    let cfg_color = headson::RenderConfig {
        color_enabled: true,
        ..cfg_plain.clone()
    };
    let prio = headson::PriorityConfig::new(usize::MAX, usize::MAX);

    // Use a tight budget so the number of kept items is sensitive to extra bytes.
    let budget = 50usize;

    let budgets = headson::Budgets {
        global: Some(headson::Budget {
            kind: headson::BudgetKind::Bytes,
            cap: budget,
        }),
        per_slot: None,
    };
    let grep = headson::GrepConfig::default();

    let plain = headson::headson(
        headson::InputKind::Json(input.to_vec()),
        &cfg_plain,
        &prio,
        &grep,
        budgets,
    )
    .expect("plain render");
    let colored = headson::headson(
        headson::InputKind::Json(input.to_vec()),
        &cfg_color,
        &prio,
        &grep,
        budgets,
    )
    .expect("color render");

    let colored_stripped = common::strip_ansi(&colored);

    // Expect identical logical output after stripping ANSI.
    assert_eq!(plain, colored_stripped);
}
