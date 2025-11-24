#[test]
fn headson_many_text_smoke() {
    // Cover the public library entrypoint for multi-file text ingest.
    use headson::{FilesetInput, FilesetInputKind};
    let cfg = headson::RenderConfig {
        template: headson::OutputTemplate::Text,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: headson::ColorMode::Off,
        color_enabled: false,
        style: headson::Style::Default,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
    };
    let prio = headson::PriorityConfig::new(100, 100);
    let inputs = vec![
        FilesetInput {
            name: "a.txt".to_string(),
            bytes: b"one\ntwo\n".to_vec(),
            kind: FilesetInputKind::Text {
                atomic_lines: false,
            },
        },
        FilesetInput {
            name: "b.log".to_string(),
            bytes: b"alpha\nbeta\n".to_vec(),
            kind: FilesetInputKind::Text {
                atomic_lines: false,
            },
        },
    ];
    let out = headson::headson(
        headson::InputKind::Fileset(inputs),
        &cfg,
        &prio,
        headson::Budgets {
            byte_budget: Some(10_000),
            char_budget: None,
            line_budget: None,
        },
    )
    .unwrap();
    assert!(out.contains("a.txt"));
    assert!(out.contains("b.log"));
    assert!(out.contains("one\n"));
    assert!(out.contains("alpha\n"));
}
