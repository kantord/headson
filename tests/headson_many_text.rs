#[test]
fn headson_many_text_smoke() {
    // Cover the public library entrypoint for multi-file text ingest.
    let cfg = headson::RenderConfig {
        template: headson::OutputTemplate::Text,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: headson::ColorMode::Off,
        color_enabled: false,
        style: headson::Style::Default,
    };
    let prio = headson::PriorityConfig::new(100, 100);
    let inputs = vec![
        ("a.txt".to_string(), b"one\ntwo\n".to_vec()),
        ("b.log".to_string(), b"alpha\nbeta\n".to_vec()),
    ];
    let out = headson::headson_many_text(inputs, &cfg, &prio, 10_000).unwrap();
    assert!(out.contains("a.txt"));
    assert!(out.contains("b.log"));
    assert!(out.contains("one\n"));
    assert!(out.contains("alpha\n"));
}
